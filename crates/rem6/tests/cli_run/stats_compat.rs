#[path = "stats_compat/cache.rs"]
mod cache;
#[path = "stats_compat/dram.rs"]
mod dram;
#[path = "stats_compat/in_order_branch_prediction_matrix.rs"]
mod in_order_branch_prediction_matrix;
#[path = "stats_compat/masked_unit_stride_vector_memory.rs"]
mod masked_unit_stride_vector_memory;
#[path = "stats_compat/selected_branch_predictor_matrix.rs"]
mod selected_branch_predictor_matrix;

use std::{env, process::Command};

use crate::support::*;
use serde_json::Value;

const M5_EXIT: u32 = 0x21;
const M5_FAIL: u32 = 0x22;
const M5_SWITCH_CPU: u32 = 0x52;

#[test]
fn rem6_run_text_stats_emit_gem5_final_tick_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-final-tick-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
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
    assert!(stdout.contains("---------- Begin Simulation Statistics ----------"));

    let final_tick = text_stat_value(&stdout, "sim.final_tick");
    assert!(final_tick > 0);
    assert_eq!(text_stat_value(&stdout, "simTicks"), final_tick);
    assert_eq!(text_stat_value(&stdout, "finalTick"), final_tick);
}

#[test]
fn rem6_run_text_stats_emit_gem5_frequency_alias() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-frequency-alias", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
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

    assert_eq!(text_stat_value(&stdout, "simFreq"), 1_000_000_000_000);
}

#[test]
fn rem6_run_text_stats_emit_gem5_instruction_alias() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-instruction-alias", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
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

    let committed = text_stat_value(&stdout, "sim.instructions.committed");
    assert_eq!(committed, 2);
    assert_eq!(text_stat_value(&stdout, "simInsts"), committed);
}

#[test]
fn rem6_run_text_stats_emit_gem5_cpu_instruction_cycle_and_rate_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-cpu-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
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

    assert_eq!(
        text_stat_value(&stdout, "system.cpu.numInsts"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.numOps"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.commitStats0.numInsts"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.commitStats0.numOps"),
        text_stat_value(&stdout, "sim.cpu0.instructions.committed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.numCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.cycles")
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.commitStats0.ipc"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.commitStats0.numInsts"),
            text_stat_value(&stdout, "system.cpu.numCycles")
        )
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.commitStats0.cpi"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.numCycles"),
            text_stat_value(&stdout, "system.cpu.commitStats0.numInsts")
        )
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.commitStats0.ipc").contains("unit=(Count/Cycle)"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.commitStats0.cpi").contains("unit=(Cycle/Count)"),
        "{stdout}"
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.ipc"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.numInsts"),
            text_stat_value(&stdout, "system.cpu.numCycles")
        )
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.cpi"),
        fixed_ratio(
            text_stat_value(&stdout, "system.cpu.numCycles"),
            text_stat_value(&stdout, "system.cpu.numInsts")
        )
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_cpu_rate_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-cpu-rate-aliases-json", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
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

    assert!(stdout.contains("\"path\":\"system.cpu.numCycles\""));
    assert!(stdout.contains("\"path\":\"system.cpu.commitStats0.numInsts\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.ipc\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.cpi\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.commitStats0.ipc\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.commitStats0.cpi\""));
}

#[test]
fn rem6_run_json_stats_emit_gem5_multicore_branch_prediction_aliases_without_ambiguous_cpu_path() {
    let program = selected_branch_predictor_matrix::selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-multicore-branch-pred-json-aliases", &elf);

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
            "--riscv-branch-predictor",
            "basic",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert!(stat_value(&stdout, "sim.cpu0.branch_predictor.btb.lookups") > 0);
    assert!(stat_value(&stdout, "sim.cpu1.branch_predictor.btb.lookups") > 0);
    for cpu in [0, 1] {
        assert!(
            stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.total")
            ) > 0
        );
        assert!(
            stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.committed.total")
            ) > 0
        );
        assert!(
            stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.total")
            ) > 0
        );
    }

    assert_gem5_branch_predictor_json_aliases_absent(&json, "system.cpu");
    for cpu in [0, 1] {
        assert_gem5_branch_predictor_json_aliases(
            &json,
            &format!("sim.cpu{cpu}"),
            &format!("system.cpu{cpu}"),
        );
    }
}

#[test]
fn rem6_run_json_stats_emit_gem5_o3_branch_prediction_aliases() {
    let path = o3_branch_prediction_alias_binary("gem5-o3-branch-pred-json-aliases");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    let predicted_taken = json_stat_value(&json, "sim.cpu0.o3.branch_event.predicted_taken");
    let mispredictions = json_stat_value(&json, "sim.cpu0.o3.branch_event.mispredictions");
    assert_eq!(predicted_taken, 1, "{stdout}");
    assert_eq!(mispredictions, 3, "{stdout}");
    assert_json_stat_alias(
        &json,
        "sim.cpu0.o3.branch_event.predicted_taken",
        "system.cpu.fetch.predictedBranches",
    );
    assert_json_stat_alias(
        &json,
        "sim.cpu0.o3.branch_event.mispredictions",
        "system.cpu.bac.branchMisspredict",
    );
    assert_json_stat_absent(&json, "system.cpu0.fetch.predictedBranches");
    assert_json_stat_absent(&json, "system.cpu0.bac.branchMisspredict");
}

#[test]
fn rem6_run_text_stats_emit_gem5_o3_branch_prediction_aliases() {
    let path = o3_branch_prediction_alias_binary("gem5-o3-branch-pred-text-aliases");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "260",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let predicted_taken = text_stat_value(&stdout, "sim.cpu0.o3.branch_event.predicted_taken");
    let mispredictions = text_stat_value(&stdout, "sim.cpu0.o3.branch_event.mispredictions");
    assert_eq!(predicted_taken, 1, "{stdout}");
    assert_eq!(mispredictions, 3, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.fetch.predictedBranches"),
        predicted_taken
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.bac.branchMisspredict"),
        mispredictions
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.fetch.predictedBranches").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.bac.branchMisspredict").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!has_text_stat(
        &stdout,
        "system.cpu0.fetch.predictedBranches"
    ));
    assert!(!has_text_stat(&stdout, "system.cpu0.bac.branchMisspredict"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_o3_lsq_store_conditional_failure_alias() {
    let path = o3_store_conditional_failure_binary("gem5-o3-lsq-sc-failure-json-alias");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();

    assert_eq!(
        json_stat_value(&json, "sim.cpu0.o3.lsq_store_conditional_failures"),
        1,
        "{stdout}"
    );
    assert_json_stat_alias(
        &json,
        "sim.cpu0.o3.lsq_store_conditional_failures",
        "system.cpu.lsq0.storeConditionalFailures",
    );
    assert_json_stat_absent(&json, "system.cpu0.lsq0.storeConditionalFailures");
}

#[test]
fn rem6_run_text_stats_emit_gem5_o3_lsq_store_conditional_failure_alias() {
    let path = o3_store_conditional_failure_binary("gem5-o3-lsq-sc-failure-text-alias");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "180",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let failures = text_stat_value(&stdout, "sim.cpu0.o3.lsq_store_conditional_failures");
    assert_eq!(failures, 1, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.lsq0.storeConditionalFailures"),
        failures
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.lsq0.storeConditionalFailures").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!has_text_stat(
        &stdout,
        "system.cpu0.lsq0.storeConditionalFailures"
    ));
}

fn assert_gem5_branch_predictor_json_aliases(
    json: &Value,
    source_prefix: &str,
    alias_prefix: &str,
) {
    for (source_suffix, alias_suffix) in [
        (
            "pipeline.in_order.conditional_branch_predictions",
            "branchPred.condPredicted",
        ),
        (
            "pipeline.in_order.conditional_branch_predicted_taken",
            "branchPred.condPredictedTaken",
        ),
        (
            "pipeline.in_order.conditional_branch_mispredictions",
            "branchPred.condIncorrect",
        ),
        ("branch_predictor.btb.lookups", "branchPred.BTBLookups"),
        (
            "branch_predictor.btb.lookups",
            "branchPred.btb.lookups::total",
        ),
        ("branch_predictor.btb.hits", "branchPred.BTBHits"),
        (
            "branch_predictor.btb.misses",
            "branchPred.btb.misses::total",
        ),
        ("branch_predictor.btb.updates", "branchPred.BTBUpdates"),
        (
            "branch_predictor.btb.updates",
            "branchPred.btb.updates::total",
        ),
        ("branch_predictor.btb.evictions", "branchPred.btb.evictions"),
        (
            "branch_predictor.btb.mispredictions",
            "branchPred.BTBMispredicted",
        ),
        (
            "branch_predictor.btb.mispredictions",
            "branchPred.btb.mispredict::total",
        ),
        (
            "branch_predictor.btb.predicted_taken_misses",
            "branchPred.predTakenBTBMiss",
        ),
        (
            "branch_predictor.btb.mispredict_due_to_btb_miss.total",
            "branchPred.mispredictDueToBTBMiss_0::total",
        ),
        (
            "branch_predictor.indirect_mispredicted",
            "branchPred.indirectMispredicted",
        ),
    ] {
        assert_json_stat_alias(
            json,
            &format!("{source_prefix}.{source_suffix}"),
            &format!("{alias_prefix}.{alias_suffix}"),
        );
    }

    for (source_kind, alias_kind) in BRANCH_TARGET_KIND_JSON_ALIASES {
        for (source_family, alias_family) in [
            ("lookups", "lookups"),
            ("misses", "misses"),
            ("updates", "updates"),
        ] {
            assert_json_stat_alias(
                json,
                &format!("{source_prefix}.branch_predictor.btb.{source_family}.{source_kind}"),
                &format!("{alias_prefix}.branchPred.btb.{alias_family}::{alias_kind}"),
            );
        }
        assert_json_stat_alias(
            json,
            &format!(
                "{source_prefix}.branch_predictor.btb.mispredict_due_to_btb_miss.{source_kind}"
            ),
            &format!("{alias_prefix}.branchPred.mispredictDueToBTBMiss_0::{alias_kind}"),
        );

        for (source_family, alias_family) in [
            ("lookups", "lookups_0"),
            ("committed", "committed_0"),
            ("mispredicted", "mispredicted_0"),
            ("corrected", "corrected_0"),
            ("target_wrong", "targetWrong_0"),
            ("mispredict_due_to_predictor", "mispredictDueToPredictor_0"),
        ] {
            assert_json_stat_alias(
                json,
                &format!("{source_prefix}.branch_predictor.{source_family}.{source_kind}"),
                &format!("{alias_prefix}.branchPred.{alias_family}::{alias_kind}"),
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
            json,
            &format!("{source_prefix}.branch_predictor.{source_family}.total"),
            &format!("{alias_prefix}.branchPred.{alias_family}::total"),
        );
    }

    for (source_provider, alias_provider) in BRANCH_TARGET_PROVIDER_JSON_ALIASES {
        assert_json_stat_alias(
            json,
            &format!("{source_prefix}.branch_predictor.target_provider.{source_provider}"),
            &format!("{alias_prefix}.branchPred.targetProvider_0::{alias_provider}"),
        );
    }
    assert_json_stat_alias(
        json,
        &format!("{source_prefix}.branch_predictor.target_provider.total"),
        &format!("{alias_prefix}.branchPred.targetProvider_0::total"),
    );

    assert_json_stat_absent(json, &format!("{alias_prefix}.branchPred.BTBHitRatio"));
}

fn assert_gem5_branch_predictor_json_aliases_absent(json: &Value, alias_prefix: &str) {
    for alias_suffix in [
        "branchPred.condPredicted",
        "branchPred.condPredictedTaken",
        "branchPred.condIncorrect",
        "branchPred.BTBLookups",
        "branchPred.btb.lookups::total",
        "branchPred.BTBHits",
        "branchPred.btb.misses::total",
        "branchPred.BTBUpdates",
        "branchPred.btb.updates::total",
        "branchPred.btb.evictions",
        "branchPred.BTBHitRatio",
        "branchPred.BTBMispredicted",
        "branchPred.btb.mispredict::total",
        "branchPred.predTakenBTBMiss",
        "branchPred.mispredictDueToBTBMiss_0::total",
        "branchPred.squashes_0::total",
        "branchPred.indirectLookups",
        "branchPred.indirectHits",
        "branchPred.indirectMisses",
        "branchPred.indirectMispredicted",
        "branchPred.ras.pushes",
        "branchPred.ras.pops",
        "branchPred.ras.squashes",
        "branchPred.ras.used",
        "branchPred.ras.correct",
        "branchPred.ras.incorrect",
    ] {
        assert_json_stat_absent(json, &format!("{alias_prefix}.{alias_suffix}"));
    }

    for (_, alias_kind) in BRANCH_TARGET_KIND_JSON_ALIASES {
        for alias_family in ["lookups", "misses", "updates"] {
            assert_json_stat_absent(
                json,
                &format!("{alias_prefix}.branchPred.btb.{alias_family}::{alias_kind}"),
            );
        }
        assert_json_stat_absent(
            json,
            &format!("{alias_prefix}.branchPred.mispredictDueToBTBMiss_0::{alias_kind}"),
        );
        assert_json_stat_absent(
            json,
            &format!("{alias_prefix}.branchPred.squashes_0::{alias_kind}"),
        );

        for alias_family in [
            "lookups_0",
            "committed_0",
            "mispredicted_0",
            "corrected_0",
            "targetWrong_0",
            "mispredictDueToPredictor_0",
        ] {
            assert_json_stat_absent(
                json,
                &format!("{alias_prefix}.branchPred.{alias_family}::{alias_kind}"),
            );
        }
    }

    for alias_family in [
        "lookups_0",
        "committed_0",
        "mispredicted_0",
        "corrected_0",
        "targetWrong_0",
        "mispredictDueToPredictor_0",
    ] {
        assert_json_stat_absent(
            json,
            &format!("{alias_prefix}.branchPred.{alias_family}::total"),
        );
    }

    for (_, alias_provider) in BRANCH_TARGET_PROVIDER_JSON_ALIASES {
        assert_json_stat_absent(
            json,
            &format!("{alias_prefix}.branchPred.targetProvider_0::{alias_provider}"),
        );
    }
    assert_json_stat_absent(
        json,
        &format!("{alias_prefix}.branchPred.targetProvider_0::total"),
    );
}

const BRANCH_TARGET_KIND_JSON_ALIASES: &[(&str, &str)] = &[
    ("no_branch", "NoBranch"),
    ("return", "Return"),
    ("call_direct", "CallDirect"),
    ("call_indirect", "CallIndirect"),
    ("direct_conditional", "DirectCond"),
    ("direct_unconditional", "DirectUncond"),
    ("indirect_conditional", "IndirectCond"),
    ("indirect_unconditional", "IndirectUncond"),
];

const BRANCH_TARGET_PROVIDER_JSON_ALIASES: &[(&str, &str)] = &[
    ("no_target", "NoTarget"),
    ("btb", "BTB"),
    ("ras", "RAS"),
    ("indirect", "Indirect"),
];

#[test]
fn rem6_run_text_stats_emit_gem5_multicore_cpu_aliases_and_rates_without_ambiguous_cpu_path() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-multicore-cpu-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    for cpu in [0, 1] {
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numInsts")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numOps")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numInsts")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numOps")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.instructions.committed"))
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.pipeline.in_order.cycles"))
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.pipeline.inOrder.stallCycles")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.pipeline.in_order.stall_cycles")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.pipeline.inOrder.resourceBlocked")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.pipeline.in_order.resource_blocked")
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.commitStats0.ipc")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numInsts")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles"))
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.commitStats0.cpi")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.commitStats0.numInsts"))
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.ipc")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numInsts")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles"))
            )
        );
        assert_eq!(
            text_stat_decimal(&stdout, &format!("system.cpu{cpu}.cpi")),
            fixed_ratio(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numCycles")),
                text_stat_value(&stdout, &format!("system.cpu{cpu}.numInsts"))
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.condPredicted")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_predictions")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.condPredictedTaken")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_predicted_taken")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.condIncorrect")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_mispredictions")
            )
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.branchPred.BTBLookups")),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.lookups")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.lookups::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.lookups")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.lookups::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.lookups.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.lookups::DirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.lookups.direct_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.branchPred.BTBHits")),
            text_stat_value(&stdout, &format!("sim.cpu{cpu}.branch_predictor.btb.hits"))
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.misses::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.misses")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.misses::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.misses.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.misses::DirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.misses.direct_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu{cpu}.branchPred.BTBUpdates")),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.updates")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.updates::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.updates")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.updates::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.updates.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.updates::DirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.updates.direct_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.evictions")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.evictions")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.BTBMispredicted")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.mispredictions")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.mispredict::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.mispredictions")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.predTakenBTBMiss")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.btb.predicted_taken_misses")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToBTBMiss_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.mispredict_due_to_btb_miss.direct_conditional"
                )
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToBTBMiss_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.mispredict_due_to_btb_miss.indirect_unconditional"
                )
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToPredictor_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.mispredict_due_to_predictor.direct_conditional"
                )
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToPredictor_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.mispredict_due_to_predictor.indirect_unconditional"
                )
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.committed_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.committed.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredicted_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.corrected_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.corrected.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.committed_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.committed.indirect_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredicted_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.indirect_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.corrected_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.corrected.indirect_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::DirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.direct_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.indirect_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.total")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetWrong_0::DirectCond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_wrong.direct_conditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetWrong_0::IndirectUncond")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_wrong.indirect_unconditional")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::BTB")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.btb")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::NoTarget")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.no_target")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.total")
            )
        );
        let btb_lookups = text_stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.btb.lookups"),
        );
        if btb_lookups != 0 {
            assert_eq!(
                text_stat_decimal(&stdout, &format!("system.cpu{cpu}.branchPred.BTBHitRatio")),
                fixed_ratio(
                    text_stat_value(&stdout, &format!("sim.cpu{cpu}.branch_predictor.btb.hits")),
                    btb_lookups
                )
            );
        }
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.condPredicted")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.condPredictedTaken")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.condIncorrect")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(&stdout, &format!("system.cpu{cpu}.branchPred.BTBLookups"))
                .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.lookups::total")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.lookups::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(&stdout, &format!("system.cpu{cpu}.branchPred.BTBHits"))
                .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.misses::total")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.misses::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(&stdout, &format!("system.cpu{cpu}.branchPred.BTBUpdates"))
                .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.updates::total")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.updates::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.evictions")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.pipeline.inOrder.stallCycles")
            )
            .contains("unit=Cycle"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.pipeline.inOrder.resourceBlocked")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.BTBMispredicted")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.btb.mispredict::total")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.predTakenBTBMiss")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToBTBMiss_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToBTBMiss_0::IndirectUncond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToPredictor_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredictDueToPredictor_0::IndirectUncond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.committed_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.mispredicted_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.corrected_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::DirectUncond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.lookups_0::total")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetWrong_0::DirectCond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetWrong_0::IndirectUncond")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::BTB")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::NoTarget")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        if btb_lookups != 0 {
            assert!(
                text_stat_line(&stdout, &format!("system.cpu{cpu}.branchPred.BTBHitRatio"))
                    .contains("unit=Ratio"),
                "{stdout}"
            );
        }
    }
    assert!(!has_text_stat(&stdout, "system.cpu.numInsts"));
    assert!(!has_text_stat(&stdout, "system.cpu.numOps"));
    assert!(!has_text_stat(&stdout, "system.cpu.numCycles"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.numInsts"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.numOps"));
    assert!(!has_text_stat(&stdout, "system.cpu.ipc"));
    assert!(!has_text_stat(&stdout, "system.cpu.cpi"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.ipc"));
    assert!(!has_text_stat(&stdout, "system.cpu.commitStats0.cpi"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.condPredicted"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.condPredictedTaken"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.condIncorrect"
    ));
    assert!(!has_text_stat(&stdout, "system.cpu.branchPred.BTBLookups"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.btb.lookups::total"
    ));
    assert!(!has_text_stat(&stdout, "system.cpu.branchPred.BTBHits"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.btb.misses::total"
    ));
    assert!(!has_text_stat(&stdout, "system.cpu.branchPred.BTBUpdates"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.btb.updates::total"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.btb.evictions"
    ));
    assert!(!has_text_stat(&stdout, "system.cpu.branchPred.BTBHitRatio"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.btb.mispredict::total"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.pipeline.inOrder.stallCycles"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.pipeline.inOrder.resourceBlocked"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.BTBMispredicted"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.predTakenBTBMiss"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.mispredictDueToBTBMiss_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.mispredictDueToBTBMiss_0::IndirectUncond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.mispredictDueToPredictor_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.mispredictDueToPredictor_0::IndirectUncond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.committed_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.mispredicted_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.corrected_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.lookups_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.lookups_0::DirectUncond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.lookups_0::total"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.targetWrong_0::DirectCond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.targetWrong_0::IndirectUncond"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.targetProvider_0::BTB"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.targetProvider_0::NoTarget"
    ));
}

#[test]
fn rem6_run_text_stats_emit_gem5_seconds_and_ops_aliases() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("gem5-seconds-ops-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
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
    assert!(stdout.starts_with("---------- Begin Simulation Statistics ----------\n"));
    assert_eq!(
        text_stat_value(&stdout, "simOps"),
        text_stat_value(&stdout, "simInsts")
    );
    assert_eq!(
        text_stat_decimal(&stdout, "simSeconds"),
        format!(
            "{:.12}",
            text_stat_value(&stdout, "finalTick") as f64 / 1_000_000_000_000_f64
        )
    );
}

#[test]
fn rem6_run_stats_emit_in_order_pipeline_cycles_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pipeline-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"committed_instructions\":2"));
    assert!(stdout.contains("\"in_order_pipeline\":{\"cycles\":10,\"in_flight\":0,"));
    assert!(stdout.contains(
        "\"stage_in_flight\":{\"fetch1\":0,\"fetch2\":0,\"decode\":0,\"execute\":0,\"commit\":0}"
    ));
    assert!(stdout.contains(
        "\"stage_max_in_flight\":{\"fetch1\":1,\"fetch2\":1,\"decode\":1,\"execute\":1,\"commit\":1}"
    ));
    assert!(stdout.contains("\"stage_occupied_cycles\":{\"fetch1\":"));
    assert!(stdout.contains("\"retired\":2"));
    assert!(stdout.contains("\"resource_blocked\":4"));
    assert!(stdout.contains("\"stall_cycles\":4"));
    assert!(stdout.contains("\"fetch_wait_cycles\":4"));
    assert!(stdout.contains("\"data_wait_cycles\":0"));
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        10,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        2,
        "monotonic",
    );
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat_greater_than(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.occupied_cycles"),
            "Cycle",
            0,
            "monotonic",
        );
    }
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.resource_blocked",
        "Count",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.stall_cycles",
        "Cycle",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.fetch_wait_cycles",
        "Cycle",
        4,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.data_wait_cycles",
        "Cycle",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_emit_configured_in_order_pipeline_widths_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pipeline-width-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-in-order-width",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"committed_instructions\":2"));
    assert!(stdout.contains(
        "\"stage_widths\":{\"fetch1\":2,\"fetch2\":2,\"decode\":2,\"execute\":2,\"commit\":2}"
    ));
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &stdout,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.width"),
            "Count",
            2,
            "constant",
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_width_changes_executed_stage_occupancy() {
    let program = riscv64_program(&[
        0x0010_0093, // addi x1, x0, 1
        0x0020_0113, // addi x2, x0, 2
        0x0030_0193, // addi x3, x0, 3
        0x0040_0213, // addi x4, x0, 4
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pipeline-width-timing", &elf);

    let width_one = in_order_pipeline_stats_for_width_and_memory(&path, 1, "direct", 80);
    let width_two = in_order_pipeline_stats_for_width_and_memory(&path, 2, "direct", 80);
    let width_two_hierarchy =
        in_order_pipeline_stats_for_width_and_memory(&path, 2, "cache-fabric-dram", 320);

    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_stat(
            &width_one,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            1,
            "monotonic",
        );
        assert_stat(
            &width_two,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            2,
            "monotonic",
        );
        assert_stat(
            &width_two_hierarchy,
            &format!("sim.cpu0.pipeline.in_order.stage.{stage}.max_in_flight"),
            "Count",
            2,
            "monotonic",
        );
    }
    assert_stat_greater_than(
        &width_two,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        stat_value(&width_two, "sim.cpu0.instructions.committed"),
        "monotonic",
    );
    for resource in [
        "sim.memory.resources.cache.instruction.activity",
        "sim.memory.resources.transport.fetch.activity",
        "sim.memory.resources.fabric.activity",
        "sim.memory.resources.dram.activity",
    ] {
        assert_stat_greater_than(&width_two_hierarchy, resource, "Count", 0, "monotonic");
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_integer_mul_execute_latency() {
    const EXPECTED_MUL_EXTRA_EXECUTE_CYCLES: u64 = 2;

    let add_stats = in_order_pipeline_latency_stats(
        "in-order-add-execute-latency",
        &[
            0x0060_0093, // addi x1, x0, 6
            0x0070_0113, // addi x2, x0, 7
            0x0020_81b3, // add x3, x1, x2
            0x0000_0073, // ecall
        ],
    );
    assert_eq!(stat_value(&add_stats, "sim.cpu0.instructions.committed"), 4);
    assert_eq!(
        stat_value(&add_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
        0
    );
    assert_eq!(
        stat_value(&add_stats, "sim.cpu0.pipeline.in_order.execute_wait_cycles"),
        0
    );

    let add_cycles = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.cycles");
    let add_stall = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    let add_execute_wait = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.execute_wait_cycles");
    for (name, word) in [
        ("mul", 0x0220_81b3),
        ("mulh", 0x0220_91b3),
        ("mulhsu", 0x0220_a1b3),
        ("mulhu", 0x0220_b1b3),
        ("mulw", 0x0220_81bb),
    ] {
        let mul_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                0x0060_0093, // addi x1, x0, 6
                0x0070_0113, // addi x2, x0, 7
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(stat_value(&mul_stats, "sim.cpu0.instructions.committed"), 4);
        assert_eq!(
            stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0
        );

        let mul_execute_wait =
            stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.execute_wait_cycles");
        assert_eq!(
            mul_execute_wait - add_execute_wait,
            EXPECTED_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should expose fixed execute-wait cycles: add={add_execute_wait}, {name}={mul_execute_wait}\nadd stats:\n{add_stats}\n{name} stats:\n{mul_stats}"
        );

        let mul_cycles = stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            mul_cycles - add_cycles,
            EXPECTED_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should consume the fixed extra execute latency: add={add_cycles}, {name}={mul_cycles}\nadd stats:\n{add_stats}\n{name} stats:\n{mul_stats}"
        );

        let mul_stall = stat_value(&mul_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            mul_stall - add_stall,
            EXPECTED_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should add the fixed execute-stage pipeline stall cycles: add={add_stall}, {name}={mul_stall}\nadd stats:\n{add_stats}\n{name} stats:\n{mul_stats}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_integer_div_rem_execute_latency() {
    const EXPECTED_DIV_EXTRA_EXECUTE_CYCLES: u64 = 19;

    let add_stats = in_order_pipeline_latency_stats(
        "in-order-div-add-baseline-execute-latency",
        &[
            0x02a0_0093, // addi x1, x0, 42
            0x0070_0113, // addi x2, x0, 7
            0x0020_81b3, // add x3, x1, x2
            0x0000_0073, // ecall
        ],
    );
    assert_eq!(stat_value(&add_stats, "sim.cpu0.instructions.committed"), 4);
    assert_eq!(
        stat_value(&add_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
        0
    );

    let add_cycles = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.cycles");
    let add_stall = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    for (name, word) in [
        ("div", 0x0220_c1b3),
        ("divu", 0x0220_d1b3),
        ("rem", 0x0220_e1b3),
        ("remu", 0x0220_f1b3),
        ("divw", 0x0220_c1bb),
        ("divuw", 0x0220_d1bb),
        ("remw", 0x0220_e1bb),
        ("remuw", 0x0220_f1bb),
    ] {
        let div_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                0x02a0_0093, // addi x1, x0, 42
                0x0070_0113, // addi x2, x0, 7
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(stat_value(&div_stats, "sim.cpu0.instructions.committed"), 4);
        assert_eq!(
            stat_value(&div_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0
        );

        let div_cycles = stat_value(&div_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            div_cycles - add_cycles,
            EXPECTED_DIV_EXTRA_EXECUTE_CYCLES,
            "{name} should consume the fixed extra execute latency: add={add_cycles}, {name}={div_cycles}\nadd stats:\n{add_stats}\n{name} stats:\n{div_stats}"
        );

        let div_stall = stat_value(&div_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            div_stall - add_stall,
            EXPECTED_DIV_EXTRA_EXECUTE_CYCLES,
            "{name} should add the fixed execute-stage pipeline stall cycles: add={add_stall}, {name}={div_stall}\nadd stats:\n{add_stats}\n{name} stats:\n{div_stats}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_scalar_fp_execute_latency() {
    let no_extra_stats = in_order_pipeline_latency_stats(
        "in-order-fp-no-extra-reference-execute-latency",
        &[
            0x0010_0293, // addi x5, x0, 1
            0x0000_0073, // ecall
        ],
    );
    assert_eq!(
        stat_value(&no_extra_stats, "sim.cpu0.instructions.committed"),
        2
    );
    assert_eq!(
        stat_value(
            &no_extra_stats,
            "sim.cpu0.pipeline.in_order.data_wait_cycles"
        ),
        0
    );

    let no_extra_stall = stat_value(&no_extra_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    let fp_reference_stats = in_order_pipeline_latency_stats(
        "in-order-fp-reference-execute-latency",
        &[
            fp_r_type(0x01, 0, 0, 0x0, 5), // fadd.d f5, f0, f0
            0x0000_0073,                   // ecall
        ],
    );
    assert_eq!(
        stat_value(&fp_reference_stats, "sim.cpu0.instructions.committed"),
        2
    );
    assert_eq!(
        stat_value(
            &fp_reference_stats,
            "sim.cpu0.pipeline.in_order.data_wait_cycles"
        ),
        0
    );

    let reference_cycles = stat_value(&fp_reference_stats, "sim.cpu0.pipeline.in_order.cycles");

    for (name, word, expected_extra_cycles) in [
        ("fadd.s", fp_r_type(0x00, 0, 0, 0x0, 5), 1),
        ("fadd.d", fp_r_type(0x01, 0, 0, 0x0, 5), 1),
        ("fsub.s", fp_r_type(0x04, 0, 0, 0x0, 5), 1),
        ("fsub.d", fp_r_type(0x05, 0, 0, 0x0, 5), 1),
        ("fmul.s", fp_r_type(0x08, 0, 0, 0x0, 5), 3),
        ("fmul.d", fp_r_type(0x09, 0, 0, 0x0, 5), 3),
        ("fmadd.s", fp_r4_type(0, 0x0, 0, 0, 0x0, 5, 0x43), 4),
        ("fmadd.d", fp_r4_type(0, 0x1, 0, 0, 0x0, 5, 0x43), 4),
        ("fmsub.s", fp_r4_type(0, 0x0, 0, 0, 0x0, 5, 0x47), 4),
        ("fmsub.d", fp_r4_type(0, 0x1, 0, 0, 0x0, 5, 0x47), 4),
        ("fnmsub.s", fp_r4_type(0, 0x0, 0, 0, 0x0, 5, 0x4b), 4),
        ("fnmsub.d", fp_r4_type(0, 0x1, 0, 0, 0x0, 5, 0x4b), 4),
        ("fnmadd.s", fp_r4_type(0, 0x0, 0, 0, 0x0, 5, 0x4f), 4),
        ("fnmadd.d", fp_r4_type(0, 0x1, 0, 0, 0x0, 5, 0x4f), 4),
        ("fdiv.s", fp_r_type(0x0c, 0, 0, 0x0, 5), 11),
        ("fdiv.d", fp_r_type(0x0d, 0, 0, 0x0, 5), 11),
        ("fsqrt.s", fp_r_type(0x2c, 0, 0, 0x0, 5), 23),
        ("fsqrt.d", fp_r_type(0x2d, 0, 0, 0x0, 5), 23),
        ("fsgnj.s", fp_r_type(0x10, 0, 0, 0x0, 5), 2),
        ("fsgnjn.s", fp_r_type(0x10, 0, 0, 0x1, 5), 2),
        ("fsgnjx.s", fp_r_type(0x10, 0, 0, 0x2, 5), 2),
        ("fsgnj.d", fp_r_type(0x11, 0, 0, 0x0, 5), 2),
        ("fsgnjn.d", fp_r_type(0x11, 0, 0, 0x1, 5), 2),
        ("fsgnjx.d", fp_r_type(0x11, 0, 0, 0x2, 5), 2),
        ("fclass.s", fp_r_type(0x70, 0, 0, 0x1, 5), 2),
        ("fclass.d", fp_r_type(0x71, 0, 0, 0x1, 5), 2),
        ("fmin.s", fp_r_type(0x14, 0, 0, 0x0, 5), 1),
        ("fmax.s", fp_r_type(0x14, 0, 0, 0x1, 5), 1),
        ("fmin.d", fp_r_type(0x15, 0, 0, 0x0, 5), 1),
        ("fmax.d", fp_r_type(0x15, 0, 0, 0x1, 5), 1),
        ("fle.s", fp_r_type(0x50, 0, 0, 0x0, 5), 1),
        ("flt.s", fp_r_type(0x50, 0, 0, 0x1, 5), 1),
        ("feq.s", fp_r_type(0x50, 0, 0, 0x2, 5), 1),
        ("fle.d", fp_r_type(0x51, 0, 0, 0x0, 5), 1),
        ("flt.d", fp_r_type(0x51, 0, 0, 0x1, 5), 1),
        ("feq.d", fp_r_type(0x51, 0, 0, 0x2, 5), 1),
        ("fmv.x.w", fp_r_type(0x70, 0, 0, 0x0, 5), 1),
        ("fmv.x.d", fp_r_type(0x71, 0, 0, 0x0, 5), 1),
        ("fmv.w.x", fp_r_type(0x78, 0, 0, 0x0, 5), 1),
        ("fmv.d.x", fp_r_type(0x79, 0, 0, 0x0, 5), 1),
        ("fcvt.s.w", fp_r_type(0x68, 0, 0, 0x0, 5), 1),
        ("fcvt.s.wu", fp_r_type(0x68, 1, 0, 0x0, 5), 1),
        ("fcvt.s.l", fp_r_type(0x68, 2, 0, 0x0, 5), 1),
        ("fcvt.s.lu", fp_r_type(0x68, 3, 0, 0x0, 5), 1),
        ("fcvt.w.s", fp_r_type(0x60, 0, 0, 0x0, 5), 1),
        ("fcvt.wu.s", fp_r_type(0x60, 1, 0, 0x0, 5), 1),
        ("fcvt.l.s", fp_r_type(0x60, 2, 0, 0x0, 5), 1),
        ("fcvt.lu.s", fp_r_type(0x60, 3, 0, 0x0, 5), 1),
        ("fcvt.s.d", fp_r_type(0x20, 1, 0, 0x0, 5), 1),
        ("fcvt.d.s", fp_r_type(0x21, 0, 0, 0x0, 5), 1),
        ("fcvt.d.w", fp_r_type(0x69, 0, 0, 0x0, 5), 1),
        ("fcvt.d.wu", fp_r_type(0x69, 1, 0, 0x0, 5), 1),
        ("fcvt.d.l", fp_r_type(0x69, 2, 0, 0x0, 5), 1),
        ("fcvt.d.lu", fp_r_type(0x69, 3, 0, 0x0, 5), 1),
        ("fcvt.w.d", fp_r_type(0x61, 0, 0, 0x0, 5), 1),
        ("fcvt.wu.d", fp_r_type(0x61, 1, 0, 0x0, 5), 1),
        ("fcvt.l.d", fp_r_type(0x61, 2, 0, 0x0, 5), 1),
        ("fcvt.lu.d", fp_r_type(0x61, 3, 0, 0x0, 5), 1),
    ] {
        let fp_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(stat_value(&fp_stats, "sim.cpu0.instructions.committed"), 2);
        assert_eq!(
            stat_value(&fp_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0
        );

        let fp_cycles = stat_value(&fp_stats, "sim.cpu0.pipeline.in_order.cycles");
        let cycle_delta = fp_cycles as i64 - reference_cycles as i64;
        assert_eq!(
            cycle_delta,
            expected_extra_cycles - 1,
            "{name} should consume fixed execute latency relative to fadd.d: reference={reference_cycles}, {name}={fp_cycles}\nreference stats:\n{fp_reference_stats}\n{name} stats:\n{fp_stats}"
        );

        let fp_stall = stat_value(&fp_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        let stall_extra = fp_stall as i64 - no_extra_stall as i64;
        assert_eq!(
            stall_extra,
            expected_extra_cycles,
            "{name} should add fixed extra execute-stage stall cycles: no-extra={no_extra_stall}, {name}={fp_stall}\nno-extra stats:\n{no_extra_stats}\n{name} stats:\n{fp_stats}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_integer_execute_latency() {
    const EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES: u64 = 2;
    const EXPECTED_VECTOR_DIV_EXTRA_EXECUTE_CYCLES: u64 = 19;
    const EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES: u64 = 1;
    const EXPECTED_VECTOR_REDUCTION_EXTRA_EXECUTE_CYCLES: u64 = 2;

    let add_stats = in_order_pipeline_latency_stats(
        "in-order-vector-add-execute-latency",
        &[
            0x0020_0513,                       // addi x10, x0, 2
            vsetvli_type(0xd0, 10, 5),         // vsetvli x5, x10, e32, m1, ta, ma
            vector_vv_type(0b000000, 2, 1, 3), // vadd.vv v3, v2, v1
            0x0000_0073,                       // ecall
        ],
    );
    assert_eq!(stat_value(&add_stats, "sim.cpu0.instructions.committed"), 4);
    assert_eq!(
        stat_value(&add_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
        0
    );

    let add_cycles = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.cycles");
    let add_stall = stat_value(&add_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    for (name, word, expected_extra_cycles) in [
        (
            "vmul.vv",
            vector_mvv_type(0b100101, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmulhu.vx",
            vector_mvx_type(0b100100, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmulhsu.vv",
            vector_mvv_type(0b100110, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmulh.vx",
            vector_mvx_type(0b100111, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmadd.vv",
            vector_mvv_type(0b101001, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmadd.vx",
            vector_mvx_type(0b101001, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnmsub.vv",
            vector_mvv_type(0b101011, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnmsub.vx",
            vector_mvx_type(0b101011, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmacc.vv",
            vector_mvv_type(0b101101, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmacc.vx",
            vector_mvx_type(0b101101, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnmsac.vv",
            vector_mvv_type(0b101111, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnmsac.vx",
            vector_mvx_type(0b101111, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vsmul.vv",
            vector_vv_type(0b100111, 2, 1, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vsmul.vx",
            vector_vx_type(0b100111, 2, 8, 3),
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vdivu.vv",
            vector_mvv_type(0b100000, 2, 1, 3),
            EXPECTED_VECTOR_DIV_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vdiv.vx",
            vector_mvx_type(0b100001, 2, 8, 3),
            EXPECTED_VECTOR_DIV_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vremu.vx",
            vector_mvx_type(0b100010, 2, 8, 3),
            EXPECTED_VECTOR_DIV_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vrem.vv",
            vector_mvv_type(0b100011, 2, 1, 3),
            EXPECTED_VECTOR_DIV_EXTRA_EXECUTE_CYCLES,
        ),
    ] {
        let vector_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                0x0020_0513,               // addi x10, x0, 2
                vsetvli_type(0xd0, 10, 5), // vsetvli x5, x10, e32, m1, ta, ma
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            4,
            "{name} should retire addi, vsetvli, the vector op, and ecall\n{name} stats:\n{vector_stats}"
        );
        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0,
            "{name} should not consume data-wait cycles\n{name} stats:\n{vector_stats}"
        );

        let vector_cycles = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            vector_cycles - add_cycles,
            expected_extra_cycles,
            "{name} should consume fixed extra execute latency: add={add_cycles}, {name}={vector_cycles}"
        );

        let vector_stall = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            vector_stall - add_stall,
            expected_extra_cycles,
            "{name} should add fixed execute-stage pipeline stall cycles: add={add_stall}, {name}={vector_stall}"
        );
    }

    let widening_add_stats = in_order_pipeline_latency_stats(
        "in-order-vector-widening-add-execute-latency",
        &[
            0x0020_0513,                                // addi x10, x0, 2
            vsetvli_type(0xd0, 10, 5),                  // vsetvli x5, x10, e32, m1, ta, ma
            vector_widening_vv_type(0b110000, 2, 1, 4), // vwaddu.vv v4, v2, v1
            0x0000_0073,                                // ecall
        ],
    );
    assert_eq!(
        stat_value(&widening_add_stats, "sim.cpu0.instructions.committed"),
        4
    );
    assert_eq!(
        stat_value(
            &widening_add_stats,
            "sim.cpu0.pipeline.in_order.data_wait_cycles"
        ),
        0
    );

    let widening_add_cycles = stat_value(&widening_add_stats, "sim.cpu0.pipeline.in_order.cycles");
    let widening_add_stall = stat_value(
        &widening_add_stats,
        "sim.cpu0.pipeline.in_order.stall_cycles",
    );
    for (name, word) in [
        ("vwmulu.vv", vector_widening_vv_type(0b111000, 2, 1, 4)),
        ("vwmulu.vx", vector_widening_vx_type(0b111000, 2, 8, 4)),
        ("vwmulsu.vv", vector_widening_vv_type(0b111010, 2, 1, 4)),
        ("vwmulsu.vx", vector_widening_vx_type(0b111010, 2, 8, 4)),
        ("vwmul.vv", vector_widening_vv_type(0b111011, 2, 1, 4)),
        ("vwmul.vx", vector_widening_vx_type(0b111011, 2, 8, 4)),
        ("vwmaccu.vv", vector_widening_vv_type(0b111100, 2, 1, 4)),
        ("vwmaccu.vx", vector_widening_vx_type(0b111100, 2, 8, 4)),
        ("vwmacc.vv", vector_widening_vv_type(0b111101, 2, 1, 4)),
        ("vwmacc.vx", vector_widening_vx_type(0b111101, 2, 8, 4)),
        ("vwmaccus.vx", vector_widening_vx_type(0b111110, 2, 8, 4)),
        ("vwmaccsu.vv", vector_widening_vv_type(0b111111, 2, 1, 4)),
        ("vwmaccsu.vx", vector_widening_vx_type(0b111111, 2, 8, 4)),
    ] {
        let vector_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                0x0020_0513,               // addi x10, x0, 2
                vsetvli_type(0xd0, 10, 5), // vsetvli x5, x10, e32, m1, ta, ma
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            4,
            "{name} should retire addi, vsetvli, the vector op, and ecall\n{name} stats:\n{vector_stats}"
        );
        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0,
            "{name} should not consume data-wait cycles\n{name} stats:\n{vector_stats}"
        );

        let vector_cycles = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            vector_cycles - widening_add_cycles,
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should consume fixed extra execute latency: widening_add={widening_add_cycles}, {name}={vector_cycles}"
        );

        let vector_stall = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            vector_stall - widening_add_stall,
            EXPECTED_VECTOR_MUL_EXTRA_EXECUTE_CYCLES,
            "{name} should add fixed execute-stage pipeline stall cycles: widening_add={widening_add_stall}, {name}={vector_stall}"
        );
    }

    let e16_add_stats = in_order_pipeline_latency_stats(
        "in-order-vector-e16-add-execute-latency",
        &[
            0x0020_0513,                       // addi x10, x0, 2
            vsetvli_type(0xc8, 10, 5),         // vsetvli x5, x10, e16, m1, ta, ma
            vector_vv_type(0b000000, 2, 1, 3), // vadd.vv v3, v2, v1
            0x0000_0073,                       // ecall
        ],
    );
    assert_eq!(
        stat_value(&e16_add_stats, "sim.cpu0.instructions.committed"),
        4
    );
    assert_eq!(
        stat_value(
            &e16_add_stats,
            "sim.cpu0.pipeline.in_order.data_wait_cycles"
        ),
        0
    );
    let e16_add_cycles = stat_value(&e16_add_stats, "sim.cpu0.pipeline.in_order.cycles");
    let e16_add_stall = stat_value(&e16_add_stats, "sim.cpu0.pipeline.in_order.stall_cycles");

    for (name, word, expected_extra_cycles) in [
        (
            "vssrl.vv",
            vector_vv_type(0b101010, 2, 1, 3),
            EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vssra.vi",
            vector_vi_type(0b101011, 2, 1, 3),
            EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnsrl.wv",
            vector_vv_type(0b101100, 6, 5, 3),
            EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnsra.wx",
            vector_vx_type(0b101101, 8, 12, 4),
            EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnclipu.wi",
            vector_vi_type(0b101110, 4, 1, 3),
            EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vnclip.wx",
            vector_vx_type(0b101111, 16, 12, 18),
            EXPECTED_VECTOR_SHIFT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vredsum.vs",
            vector_reduction_type(0b000000, 2, 1, 3),
            EXPECTED_VECTOR_REDUCTION_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vredmax.vs",
            vector_reduction_type(0b000111, 2, 1, 3),
            EXPECTED_VECTOR_REDUCTION_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vwredsumu.vs",
            vector_widening_reduction_type(0b110000, 2, 1, 3),
            EXPECTED_VECTOR_REDUCTION_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vwredsum.vs",
            vector_widening_reduction_type(0b110001, 2, 1, 3),
            EXPECTED_VECTOR_REDUCTION_EXTRA_EXECUTE_CYCLES,
        ),
    ] {
        let vector_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &[
                0x0020_0513,               // addi x10, x0, 2
                vsetvli_type(0xc8, 10, 5), // vsetvli x5, x10, e16, m1, ta, ma
                word,
                0x0000_0073, // ecall
            ],
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            4,
            "{name} should retire addi, vsetvli, the vector op, and ecall\n{name} stats:\n{vector_stats}"
        );
        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0,
            "{name} should not consume data-wait cycles\n{name} stats:\n{vector_stats}"
        );

        let vector_cycles = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            vector_cycles - e16_add_cycles,
            expected_extra_cycles,
            "{name} should consume fixed extra execute latency: e16_add={e16_add_cycles}, {name}={vector_cycles}"
        );

        let vector_stall = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            vector_stall - e16_add_stall,
            expected_extra_cycles,
            "{name} should add fixed execute-stage pipeline stall cycles: e16_add={e16_add_stall}, {name}={vector_stall}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_fp_execute_latency() {
    const EXPECTED_VECTOR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES: u64 = 1;
    const EXPECTED_VECTOR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES: u64 = 1;
    const EXPECTED_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES: u64 = 1;
    const EXPECTED_VECTOR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES: u64 = 2;
    const EXPECTED_VECTOR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES: u64 = 3;
    const EXPECTED_VECTOR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES: u64 = 4;
    const EXPECTED_VECTOR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES: u64 = 11;
    const EXPECTED_VECTOR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES: u64 = 23;

    let no_extra_stats = in_order_pipeline_latency_stats(
        "in-order-vector-fp-no-extra-execute-latency",
        &vector_fp_latency_program(vector_vv_type(0b000000, 2, 1, 3)), // vadd.vv v3, v2, v1
    );
    assert_eq!(
        stat_value(&no_extra_stats, "sim.cpu0.instructions.committed"),
        8
    );
    assert_eq!(
        stat_value(
            &no_extra_stats,
            "sim.cpu0.pipeline.in_order.data_wait_cycles"
        ),
        0
    );

    let no_extra_cycles = stat_value(&no_extra_stats, "sim.cpu0.pipeline.in_order.cycles");
    let no_extra_stall = stat_value(&no_extra_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    for (name, word, expected_extra_cycles) in [
        (
            "vfadd.vv",
            vector_float_vv_type(0b000000, 2, 1, 3),
            EXPECTED_VECTOR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfsub.vf",
            vector_float_vf_type(0b000010, 2, 8, 3),
            EXPECTED_VECTOR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfrsub.vf",
            vector_float_vf_type(0b100111, 2, 8, 3),
            EXPECTED_VECTOR_FLOAT_ADD_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfmin.vv",
            vector_float_vv_type(0b000100, 2, 1, 3),
            EXPECTED_VECTOR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vmfeq.vf",
            vector_float_vf_type(0b011000, 2, 8, 3),
            EXPECTED_VECTOR_FLOAT_CMP_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfcvt.xu.f.v",
            vector_float_vv_type(0b010010, 1, 0x00, 3),
            EXPECTED_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfcvt.f.xu.v",
            vector_float_vv_type(0b010010, 1, 0x02, 3),
            EXPECTED_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfmv.v.f",
            vector_float_vf_type(0b010111, 0, 8, 3),
            EXPECTED_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfmerge.vfm",
            vector_float_masked_type(0b010111, 0b101, 2, 8, 3),
            EXPECTED_VECTOR_FLOAT_CVT_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfsgnj.vv",
            vector_float_vv_type(0b001000, 2, 1, 3),
            EXPECTED_VECTOR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfclass.v",
            vector_float_type(0b010011, 0b001, 1, 0x10, 3),
            EXPECTED_VECTOR_FLOAT_MISC_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfmul.vf",
            vector_float_vf_type(0b100100, 2, 8, 3),
            EXPECTED_VECTOR_FLOAT_MUL_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfmacc.vv",
            vector_float_vv_type(0b101100, 2, 1, 3),
            EXPECTED_VECTOR_FLOAT_MUL_ADD_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfdiv.vv",
            vector_float_vv_type(0b100000, 2, 1, 3),
            EXPECTED_VECTOR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfrdiv.vf",
            vector_float_vf_type(0b100001, 2, 8, 3),
            EXPECTED_VECTOR_FLOAT_DIV_EXTRA_EXECUTE_CYCLES,
        ),
        (
            "vfsqrt.v",
            vector_float_type(0b010011, 0b001, 1, 0x00, 3),
            EXPECTED_VECTOR_FLOAT_SQRT_EXTRA_EXECUTE_CYCLES,
        ),
    ] {
        let vector_stats = in_order_pipeline_latency_stats(
            &format!("in-order-{name}-execute-latency"),
            &vector_fp_latency_program(word),
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            8
        );
        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            0
        );

        let vector_cycles = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.cycles");
        assert_eq!(
            vector_cycles - no_extra_cycles,
            expected_extra_cycles,
            "{name} should consume fixed extra execute latency: no-extra={no_extra_cycles}, {name}={vector_cycles}"
        );

        let vector_stall = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
        assert_eq!(
            vector_stall - no_extra_stall,
            expected_extra_cycles,
            "{name} should add fixed execute-stage pipeline stall cycles: no-extra={no_extra_stall}, {name}={vector_stall}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_load_store_latency() {
    const EXPECTED_VECTOR_LOAD_EXTRA_EXECUTE_CYCLES: u64 = 2;
    const EXPECTED_VECTOR_STORE_EXTRA_EXECUTE_CYCLES: u64 = 1;
    const EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES: u64 =
        EXPECTED_VECTOR_LOAD_EXTRA_EXECUTE_CYCLES + EXPECTED_VECTOR_STORE_EXTRA_EXECUTE_CYCLES;

    let scalar_stats = in_order_pipeline_payload_stats(
        "in-order-scalar-ld-sd-vector-memory-baseline",
        &unit_stride_memory_program(false),
    );
    let vector_stats = in_order_pipeline_payload_stats(
        "in-order-vector-unit-stride-load-store-latency",
        &unit_stride_memory_program(true),
    );

    for (name, stats) in [("scalar", &scalar_stats), ("vector", &vector_stats)] {
        assert_eq!(
            stat_value(stats, "sim.cpu0.instructions.committed"),
            14,
            "{name} program should retire through the success ecall\n{name} stats:\n{stats}"
        );
    }

    let scalar_data_wait = stat_value(&scalar_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles");
    let vector_data_wait = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles");
    assert_eq!(
        vector_data_wait, scalar_data_wait,
        "vector unit-stride load/store should reuse the same direct-memory data wait as ld/sd"
    );

    let scalar_execute_wait = stat_value(
        &scalar_stats,
        "sim.cpu0.pipeline.in_order.execute_wait_cycles",
    );
    let vector_execute_wait = stat_value(
        &vector_stats,
        "sim.cpu0.pipeline.in_order.execute_wait_cycles",
    );
    assert_eq!(
        vector_execute_wait - scalar_execute_wait,
        EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
        "vector unit-stride load/store should add fixed LSU execute latency"
    );

    let scalar_cycles = stat_value(&scalar_stats, "sim.cpu0.pipeline.in_order.cycles");
    let vector_cycles = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.cycles");
    assert_eq!(
        vector_cycles - scalar_cycles,
        EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
        "vector unit-stride load/store should add fixed pipeline cycles"
    );

    let scalar_stall = stat_value(&scalar_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    let vector_stall = stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.stall_cycles");
    assert_eq!(
        vector_stall - scalar_stall,
        EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
        "vector unit-stride load/store should add fixed execute-stage stall cycles"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_load_store_element_widths() {
    const EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES: u64 = 3;

    let scalar_stats = in_order_pipeline_payload_stats(
        "in-order-scalar-ld-sd-vector-memory-width-baseline",
        &unit_stride_memory_program(false),
    );
    let scalar_data_wait = stat_value(&scalar_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles");
    let scalar_execute_wait = stat_value(
        &scalar_stats,
        "sim.cpu0.pipeline.in_order.execute_wait_cycles",
    );

    for (name, vtype, avl, load, store) in [
        (
            "e8",
            0xc0,
            8,
            0x0205_0087, // vle8.v v1, (x10)
            0x0208_00a7, // vse8.v v1, (x16)
        ),
        (
            "e16",
            0xc8,
            4,
            0x0205_5087, // vle16.v v1, (x10)
            0x0208_50a7, // vse16.v v1, (x16)
        ),
        (
            "e64",
            0xd8,
            1,
            0x0205_7087, // vle64.v v1, (x10)
            0x0208_70a7, // vse64.v v1, (x16)
        ),
    ] {
        let vector_stats = in_order_pipeline_payload_stats(
            &format!("in-order-vector-unit-stride-load-store-{name}"),
            &unit_stride_vector_memory_program(vtype, avl, load, store),
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            14,
            "{name} unit-stride vector memory should retire through the success ecall\nstats:\n{vector_stats}"
        );
        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
            scalar_data_wait,
            "{name} unit-stride vector memory should issue the same 8-byte data footprint as ld/sd"
        );
        assert_eq!(
            stat_value(
                &vector_stats,
                "sim.cpu0.pipeline.in_order.execute_wait_cycles"
            ) - scalar_execute_wait,
            EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
            "{name} unit-stride vector memory should add fixed vector LSU execute latency"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_fault_only_e8_e16_e32_e64_m1_load_memory() {
    const EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES: u64 = 3;

    let scalar_stats = in_order_pipeline_payload_stats(
        "in-order-scalar-ld-sd-vector-fault-only-baseline",
        &unit_stride_memory_program(false),
    );
    let scalar_execute_wait = stat_value(
        &scalar_stats,
        "sim.cpu0.pipeline.in_order.execute_wait_cycles",
    );
    for (name, vtype, avl, width) in [
        ("e8", 0xc0, 8, 0b000),
        ("e16", 0xc8, 4, 0b101),
        ("e32", 0xd0, 2, 0b110),
        ("e64", 0xd8, 1, 0b111),
    ] {
        let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
            &format!("in-order-vector-fault-only-{name}-m1-load"),
            &unit_stride_vector_memory_program(
                vtype,
                avl,
                vector_unit_stride_fault_only_load_type(true, width, 10, 1),
                vector_unit_stride_store_type(true, width, 16, 1),
            ),
            100,
        );

        assert_eq!(
            stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
            14,
            "no-fault {name}/m1 fault-only-first vector load should retire through the direct-memory top-level run path\nstats:\n{direct_stats}"
        );
        assert_eq!(
            stat_value(
                &direct_stats,
                "sim.cpu0.pipeline.in_order.execute_wait_cycles"
            ) - scalar_execute_wait,
            EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
            "no-fault {name}/m1 fault-only-first vector load should add fixed vector LSU execute latency\nstats:\n{direct_stats}"
        );

        let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
            &format!("in-order-cache-vector-fault-only-{name}-m1-load"),
            &unit_stride_vector_memory_program(
                vtype,
                avl,
                vector_unit_stride_fault_only_load_type(true, width, 10, 1),
                vector_unit_stride_store_type(true, width, 16, 1),
            ),
            500,
        );

        assert_eq!(
            stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
            14,
            "no-fault {name}/m1 fault-only-first vector load should retire through the cache-backed top-level run path\nstats:\n{cache_stats}"
        );
        assert_eq!(
            stat_value(
                &cache_stats,
                "sim.cpu0.pipeline.in_order.execute_wait_cycles"
            ) - scalar_execute_wait,
            EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
            "no-fault {name}/m1 fault-only-first vector load should add fixed vector LSU execute latency through the cache-backed top-level run path\nstats:\n{cache_stats}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_fault_only_first_later_line_fault() {
    let stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-fault-only-first-e32-later-line-fault",
        &fault_only_first_later_line_fault_program(),
        160,
    );

    assert_eq!(
        simulation_trap(&stats).as_deref(),
        Some("environment_call"),
        "fault-only-first e32 later-line fault should reach the guest ecall instead of a data fault\nstats:\n{stats}"
    );
    assert_eq!(
        stat_value(&stats, "sim.cpu0.instructions.committed"),
        20,
        "fault-only-first e32 load should retire after loading the first lane, suppressing the later unmapped-line lane and reducing vl for the following store\nstats:\n{stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_fault_only_first_inactive_later_line() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-fault-only-first-masked-e32-inactive-later-line",
        &masked_fault_only_first_inactive_later_line_program(),
        220,
    );

    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "masked fault-only-first e32 load should suppress the inactive later unmapped-line lane and reach the direct-memory success ecall\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        21,
        "masked fault-only-first e32 load should load lane 0, preserve lane 1, and keep vl for the following store through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-fault-only-first-masked-e32-inactive-later-line",
        &masked_fault_only_first_inactive_later_line_program(),
        700,
    );

    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed masked fault-only-first e32 load should suppress the inactive later unmapped-line lane and reach the success ecall\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        21,
        "cache-backed masked fault-only-first e32 load should load lane 0, preserve lane 1, and keep vl for the following store through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_fault_only_first_active_later_line() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-fault-only-first-masked-e32-active-later-line",
        &masked_fault_only_first_active_later_line_program(),
        240,
    );

    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "masked fault-only-first e32 load should suppress the active later unmapped-line lane after loading lane 0 and reach the direct-memory success ecall\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        23,
        "masked fault-only-first e32 load should reduce vl before the following direct-memory store when the active later lane faults\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-fault-only-first-masked-e32-active-later-line",
        &masked_fault_only_first_active_later_line_program(),
        760,
    );

    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed masked fault-only-first e32 load should suppress the active later unmapped-line lane after loading lane 0 and reach the success ecall\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        23,
        "cache-backed masked fault-only-first e32 load should reduce vl before the following store when the active later lane faults\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_fault_only_e32_m2_full_register_group_memory() {
    let normal_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-lmul2-load-store-fault-only-baseline",
        &unit_stride_lmul2_vector_memory_program(8),
        120,
    );
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-fault-only-e32-m2-full-load",
        &fault_only_lmul2_vector_memory_program(8),
        120,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        32,
        "no-fault e32/m2 fault-only-first vector load should move a full 32-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(
            &direct_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        stat_value(
            &normal_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        "no-fault e32/m2 fault-only-first vector load should reuse normal e32/m2 vector LSU execute latency\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-fault-only-e32-m2-full-load",
        &fault_only_lmul2_vector_memory_program(8),
        500,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        32,
        "no-fault e32/m2 fault-only-first vector load should move a full 32-byte register-group payload through the cache-backed top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(
            &cache_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        stat_value(
            &normal_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        "cache-backed no-fault e32/m2 fault-only-first vector load should reuse normal e32/m2 vector LSU execute latency\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_fault_only_e32_m4_full_register_group_memory() {
    let normal_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-lmul4-load-store-fault-only-baseline",
        &unit_stride_lmul4_vector_memory_program(),
        260,
    );
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-fault-only-e32-m4-full-load",
        &fault_only_lmul4_vector_memory_program(),
        260,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        56,
        "no-fault e32/m4 fault-only-first vector load should move a full 64-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(
            &direct_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        stat_value(
            &normal_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        "no-fault e32/m4 fault-only-first vector load should reuse normal e32/m4 vector LSU execute latency\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-fault-only-e32-m4-full-load",
        &fault_only_lmul4_vector_memory_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        56,
        "no-fault e32/m4 fault-only-first vector load should move a full 64-byte register-group payload through the cache-backed top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(
            &cache_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        stat_value(
            &normal_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        "cache-backed no-fault e32/m4 fault-only-first vector load should reuse normal e32/m4 vector LSU execute latency\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_fault_only_e32_m8_full_register_group_memory() {
    let normal_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-lmul8-load-store-fault-only-baseline",
        &unit_stride_lmul8_vector_memory_program(),
        520,
    );
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-fault-only-e32-m8-full-load",
        &fault_only_lmul8_vector_memory_program(),
        520,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        104,
        "no-fault e32/m8 fault-only-first vector load should move a full 128-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        stat_value(
            &direct_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        stat_value(
            &normal_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        "no-fault e32/m8 fault-only-first vector load should reuse normal e32/m8 vector LSU execute latency\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-fault-only-e32-m8-full-load",
        &fault_only_lmul8_vector_memory_program(),
        1800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        104,
        "no-fault e32/m8 fault-only-first vector load should move a full 128-byte register-group payload through the cache-backed top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        stat_value(
            &cache_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        stat_value(
            &normal_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        "cache-backed no-fault e32/m8 fault-only-first vector load should reuse normal e32/m8 vector LSU execute latency\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_lmul2_register_group_memory() {
    const EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES: u64 = 3;

    let scalar_stats = in_order_pipeline_payload_stats(
        "in-order-scalar-ld-sd-vector-lmul2-memory-baseline",
        &unit_stride_memory_program(false),
    );
    let vector_stats = in_order_pipeline_payload_stats(
        "in-order-vector-unit-stride-lmul2-load-store",
        &unit_stride_lmul2_vector_memory_program(4),
    );

    assert_eq!(
        stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
        20,
        "LMUL2 unit-stride vector memory should retire through the success ecall\nstats:\n{vector_stats}"
    );
    assert_eq!(
        stat_value(
            &vector_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ) - stat_value(
            &scalar_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        EXPECTED_VECTOR_MEMORY_EXTRA_EXECUTE_CYCLES,
        "LMUL2 vector memory should reuse fixed vector LSU execute latency\nstats:\n{vector_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_full_e16_lmul2_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-e16-lmul2-load-store",
        &unit_stride_e16_lmul2_vector_memory_program(),
        260,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        56,
        "e16/m2 unit-stride vector memory should move a full 32-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-full-e16-lmul2-load-store",
        &unit_stride_e16_lmul2_vector_memory_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        56,
        "cache-backed e16/m2 unit-stride vector memory should move a full 32-byte register-group payload through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_full_e16_lmul4_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-e16-lmul4-load-store",
        &unit_stride_e16_lmul4_vector_memory_program(),
        520,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        104,
        "e16/m4 unit-stride vector memory should move a full 64-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "e16/m4 unit-stride vector memory should reach the success ecall after guest-side halfword movement checks\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-full-e16-lmul4-load-store",
        &unit_stride_e16_lmul4_vector_memory_program(),
        1800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        104,
        "cache-backed e16/m4 unit-stride vector memory should move a full 64-byte register-group payload through the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed e16/m4 unit-stride vector memory should reach the success ecall after guest-side halfword movement checks\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_full_lmul2_register_group_memory() {
    let vector_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-lmul2-load-store",
        &unit_stride_lmul2_vector_memory_program(8),
        120,
    );

    assert_eq!(
        stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
        32,
        "LMUL2 unit-stride vector memory should move a full 32-byte register-group payload through the top-level run path\nstats:\n{vector_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_cache_backed_vector_unit_stride_full_lmul2_register_group_memory(
) {
    let vector_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-full-lmul2-load-store",
        &unit_stride_lmul2_vector_memory_program(8),
        500,
    );

    assert_eq!(
        stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
        32,
        "cache-backed LMUL2 unit-stride vector memory should move a full 32-byte register-group payload through the top-level run path\nstats:\n{vector_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_segment_e32_m1_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-e32-m1-load-store",
        &unit_stride_segment_e32_m1_vector_memory_program(),
        120,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        22,
        "e32/m1 segment load/store should scatter two fields into vector registers and gather them back through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-e32-m1-load-store",
        &unit_stride_segment_e32_m1_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        22,
        "cache-backed e32/m1 segment load/store should scatter two fields into vector registers and gather them back through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_segment_e32_m1_three_field_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-e32-m1-three-field-load-store",
        &unit_stride_segment_e32_m1_three_field_vector_memory_program(),
        140,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        19,
        "three-field e32/m1 segment load/store should scatter three fields into vector registers and gather them back through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-e32-m1-three-field-load-store",
        &unit_stride_segment_e32_m1_three_field_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        19,
        "cache-backed three-field e32/m1 segment load/store should scatter three fields into vector registers and gather them back through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_segment_e32_m1_four_field_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-e32-m1-four-field-load-store",
        &unit_stride_segment_e32_m1_four_field_vector_memory_program(),
        180,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "four-field e32/m1 segment load/store should scatter four fields into vector registers and gather them back through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "four-field e32/m1 segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-e32-m1-four-field-load-store",
        &unit_stride_segment_e32_m1_four_field_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed four-field e32/m1 segment load/store should scatter four fields into vector registers and gather them back through the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed four-field e32/m1 segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_segment_e8_e16_e64_m1_load_store() {
    for (name, program, committed, direct_tick, cache_tick) in [
        (
            "e8",
            unit_stride_segment_e8_m1_vector_memory_program(),
            22,
            120,
            600,
        ),
        (
            "e16",
            unit_stride_segment_e16_m1_vector_memory_program(),
            22,
            120,
            600,
        ),
        (
            "e64",
            unit_stride_segment_e64_m1_vector_memory_program(),
            16,
            140,
            600,
        ),
        (
            "e64-two-line",
            unit_stride_segment_e64_m1_two_line_vector_memory_program(),
            22,
            160,
            700,
        ),
    ] {
        let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
            &format!("in-order-vector-segment-{name}-m1-load-store"),
            &program,
            direct_tick,
        );

        assert_eq!(
            stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
            committed,
            "{name}/m1 segment load/store should scatter two fields and gather them back through the direct-memory top-level run path\nstats:\n{direct_stats}"
        );

        let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
            &format!("in-order-cache-vector-segment-{name}-m1-load-store"),
            &program,
            cache_tick,
        );

        assert_eq!(
            stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
            committed,
            "cache-backed {name}/m1 segment load/store should scatter two fields and gather them back through the top-level run path\nstats:\n{cache_stats}"
        );
    }
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_segment_e16_m2_full_register_group_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-e16-m2-full-register-group-load-store",
        &unit_stride_segment_e16_m2_full_register_group_vector_memory_program(),
        520,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        106,
        "e16/m2 full-register-group segment load/store should scatter two 32-byte fields into vector register groups and gather them back through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "e16/m2 full-register-group segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-e16-m2-full-register-group-load-store",
        &unit_stride_segment_e16_m2_full_register_group_vector_memory_program(),
        1600,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        106,
        "cache-backed e16/m2 full-register-group segment load/store should scatter two 32-byte fields into vector register groups and gather them back through the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed e16/m2 full-register-group segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_segment_e32_m1_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-masked-e32-m1-load-store",
        &masked_unit_stride_segment_e32_m1_vector_memory_program(),
        240,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        47,
        "masked e32/m1 segment load/store should preserve inactive field lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-masked-e32-m1-load-store",
        &masked_unit_stride_segment_e32_m1_vector_memory_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        47,
        "cache-backed masked e32/m1 segment load/store should preserve inactive field lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_segment_e16_m1_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-masked-e16-m1-load-store",
        &masked_unit_stride_segment_e16_m1_vector_memory_program(),
        240,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        47,
        "masked e16/m1 segment load/store should preserve inactive field lanes and skipped store halfwords through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "masked e16/m1 segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-masked-e16-m1-load-store",
        &masked_unit_stride_segment_e16_m1_vector_memory_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        47,
        "cache-backed masked e16/m1 segment load/store should preserve inactive field lanes and skipped store halfwords through the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed masked e16/m1 segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_segment_e64_m1_two_line_load_store() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-segment-masked-e64-m1-two-line-load-store",
        &masked_unit_stride_segment_e64_m1_two_line_vector_memory_program(),
        300,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        47,
        "masked e64/m1 two-line segment load/store should preserve inactive field lanes and skipped store doublewords through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );
    assert_eq!(
        simulation_trap(&direct_stats).as_deref(),
        Some("environment_call"),
        "masked e64/m1 two-line segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-segment-masked-e64-m1-two-line-load-store",
        &masked_unit_stride_segment_e64_m1_two_line_vector_memory_program(),
        900,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        47,
        "cache-backed masked e64/m1 two-line segment load/store should preserve inactive field lanes and skipped store doublewords through the top-level run path\nstats:\n{cache_stats}"
    );
    assert_eq!(
        simulation_trap(&cache_stats).as_deref(),
        Some("environment_call"),
        "cache-backed masked e64/m1 two-line segment load/store should reach the success ecall, not the invalid-instruction failure path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_full_lmul4_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-lmul4-load-store",
        &unit_stride_lmul4_vector_memory_program(),
        260,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        56,
        "LMUL4 unit-stride vector memory should move a full 64-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-full-lmul4-load-store",
        &unit_stride_lmul4_vector_memory_program(),
        800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        56,
        "cache-backed LMUL4 unit-stride vector memory should move a full 64-byte register-group payload through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_unit_stride_full_lmul8_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-full-lmul8-load-store",
        &unit_stride_lmul8_vector_memory_program(),
        520,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        104,
        "LMUL8 unit-stride vector memory should move a full 128-byte register-group payload through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-full-lmul8-load-store",
        &unit_stride_lmul8_vector_memory_program(),
        1800,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        104,
        "cache-backed LMUL8 unit-stride vector memory should move a full 128-byte register-group payload through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e8_m1_stride15_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e8-m1-stride15-load-store",
        &strided_e8_m1_stride15_vector_memory_program(),
        400,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        29,
        "two-lane stride-15 e8,m1 vector memory should move compact byte lanes and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e8-m1-stride15-load-store",
        &strided_e8_m1_stride15_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        29,
        "cache-backed two-lane stride-15 e8,m1 vector memory should move compact byte lanes and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e8_m1_stride15_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e8-m1-stride15-load-store",
        &masked_strided_e8_m1_stride15_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "masked two-lane stride-15 e8,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e8-m1-stride15-load-store",
        &masked_strided_e8_m1_stride15_vector_memory_program(),
        1280,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed masked two-lane stride-15 e8,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e8_m1_stride7_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e8-m1-stride7-load-store",
        &strided_e8_m1_stride7_vector_memory_program(),
        400,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        29,
        "two-lane stride-7 e8,m1 vector memory should move gapped byte lanes and preserve gap store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e8-m1-stride7-load-store",
        &strided_e8_m1_stride7_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        29,
        "cache-backed two-lane stride-7 e8,m1 vector memory should move gapped byte lanes and preserve gap store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e8_m1_stride7_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e8-m1-stride7-load-store",
        &masked_strided_e8_m1_stride7_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "masked two-lane stride-7 e8,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e8-m1-stride7-load-store",
        &masked_strided_e8_m1_stride7_vector_memory_program(),
        1280,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed masked two-lane stride-7 e8,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e8_m1_stride3_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e8-m1-stride3-load-store",
        &strided_e8_m1_stride3_vector_memory_program(),
        400,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        29,
        "two-lane stride-3 e8,m1 vector memory should move gapped byte lanes and preserve gap store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e8-m1-stride3-load-store",
        &strided_e8_m1_stride3_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        29,
        "cache-backed two-lane stride-3 e8,m1 vector memory should move gapped byte lanes and preserve gap store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e8_m1_stride3_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e8-m1-stride3-load-store",
        &masked_strided_e8_m1_stride3_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "masked two-lane stride-3 e8,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e8-m1-stride3-load-store",
        &masked_strided_e8_m1_stride3_vector_memory_program(),
        1280,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed masked two-lane stride-3 e8,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e16_m1_stride14_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e16-m1-stride14-load-store",
        &strided_e16_m1_stride14_vector_memory_program(),
        400,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        29,
        "two-lane stride-14 e16,m1 vector memory should move compact halfword lanes and preserve skipped store halfwords through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e16-m1-stride14-load-store",
        &strided_e16_m1_stride14_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        29,
        "cache-backed two-lane stride-14 e16,m1 vector memory should move compact halfword lanes and preserve skipped store halfwords through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e16_m1_stride14_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e16-m1-stride14-load-store",
        &masked_strided_e16_m1_stride14_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "masked two-lane stride-14 e16,m1 vector memory should preserve inactive compact lanes and skipped store halfwords through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e16-m1-stride14-load-store",
        &masked_strided_e16_m1_stride14_vector_memory_program(),
        1280,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed masked two-lane stride-14 e16,m1 vector memory should preserve inactive compact lanes and skipped store halfwords through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e16_m1_stride6_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e16-m1-stride6-load-store",
        &strided_e16_m1_stride6_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        29,
        "two-lane stride-6 e16,m1 vector memory should move gapped halfword lanes and preserve gap store halfwords through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e16-m1-stride6-load-store",
        &strided_e16_m1_stride6_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        29,
        "cache-backed two-lane stride-6 e16,m1 vector memory should move gapped halfword lanes and preserve gap store halfwords through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e16_m1_stride6_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e16-m1-stride6-load-store",
        &masked_strided_e16_m1_stride6_vector_memory_program(),
        560,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "masked two-lane stride-6 e16,m1 vector memory should preserve inactive compact halfword lanes and skipped store halfwords through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e16-m1-stride6-load-store",
        &masked_strided_e16_m1_stride6_vector_memory_program(),
        1400,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed masked two-lane stride-6 e16,m1 vector memory should preserve inactive compact halfword lanes and skipped store halfwords through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e32-m1-load-store",
        &strided_e32_m1_vector_memory_program(),
        120,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        21,
        "strided e32,m1 vector memory should move selected lanes and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e32-m1-load-store",
        &strided_e32_m1_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        21,
        "cache-backed strided e32,m1 vector memory should move selected lanes and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e32-m1-load-store",
        &masked_strided_e32_m1_vector_memory_program(),
        220,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        37,
        "masked strided e32,m1 vector memory should preserve inactive lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e32-m1-load-store",
        &masked_strided_e32_m1_vector_memory_program(),
        640,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        37,
        "cache-backed masked strided e32,m1 vector memory should preserve inactive lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e32_m1_stride6_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e32-m1-stride6-load-store",
        &strided_e32_m1_stride6_vector_memory_program(),
        260,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "three-lane stride-6 e32,m1 vector memory should move unaligned strided lanes and preserve gap store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e32-m1-stride6-load-store",
        &strided_e32_m1_stride6_vector_memory_program(),
        700,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed three-lane stride-6 e32,m1 vector memory should move unaligned strided lanes and preserve gap store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e32_m1_stride6_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e32-m1-stride6-load-store",
        &masked_strided_e32_m1_stride6_vector_memory_program(),
        340,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        44,
        "masked three-lane stride-6 e32,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e32-m1-stride6-load-store",
        &masked_strided_e32_m1_stride6_vector_memory_program(),
        940,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        44,
        "cache-backed masked three-lane stride-6 e32,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_strided_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-e64-m1-load-store",
        &strided_e64_m1_vector_memory_program(),
        220,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        26,
        "strided e64,m1 vector memory should move two 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-e64-m1-load-store",
        &strided_e64_m1_vector_memory_program(),
        620,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        26,
        "cache-backed strided e64,m1 vector memory should move two 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_vector_strided_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-sparse-e64-m1-load-store",
        &sparse_strided_e64_m1_vector_memory_program(),
        400,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        32,
        "sparse strided e64,m1 vector memory should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-sparse-e64-m1-load-store",
        &sparse_strided_e64_m1_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        32,
        "cache-backed sparse strided e64,m1 vector memory should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_strided_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-e64-m1-load-store",
        &masked_strided_e64_m1_vector_memory_program(),
        280,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "masked strided e64,m1 vector memory should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-e64-m1-load-store",
        &masked_strided_e64_m1_vector_memory_program(),
        760,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed masked strided e64,m1 vector memory should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_vector_strided_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-strided-masked-sparse-e64-m1-load-store",
        &masked_sparse_strided_e64_m1_vector_memory_program(),
        500,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        37,
        "masked sparse strided e64,m1 vector memory should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-strided-masked-sparse-e64-m1-load-store",
        &masked_sparse_strided_e64_m1_vector_memory_program(),
        1280,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        37,
        "cache-backed masked sparse strided e64,m1 vector memory should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e8_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e8-m1-load-store",
        &indexed_e8_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        30,
        "indexed e8,m1 vector memory should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e8-m1-load-store",
        &indexed_e8_m1_vector_memory_program(),
        1040,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        30,
        "cache-backed indexed e8,m1 vector memory should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e8_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e8-m1-load-store",
        &masked_indexed_e8_m1_vector_memory_program(),
        560,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        35,
        "masked indexed e8,m1 vector memory should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e8-m1-load-store",
        &masked_indexed_e8_m1_vector_memory_program(),
        1400,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        35,
        "cache-backed masked indexed e8,m1 vector memory should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e8_m1_data_e16_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e8-data-e16-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e8,m1 data with e16 indices should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e8-data-e16-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e8,m1 data with e16 indices should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e8_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e8-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e8,m1 data with e16 indices should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e8-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e8,m1 data with e16 indices should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e8_m1_data_e32_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e8-data-e32-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e8,m1 data with e32 indices should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e8-data-e32-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e8,m1 data with e32 indices should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e8_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e8-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e8,m1 data with e32 indices should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e8-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e8,m1 data with e32 indices should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e8_m1_data_e64_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e8-data-e64-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e8,m1 data with e64 indices should move selected byte offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e8-data-e64-indices-load-store",
        &mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e8,m1 data with e64 indices should move selected byte offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e8_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e8-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e8,m1 data with e64 indices should preserve inactive compact byte lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e8-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e8,m1 data with e64 indices should preserve inactive compact byte lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e16_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e16-m1-load-store",
        &indexed_e16_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        30,
        "indexed e16,m1 vector memory should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e16-m1-load-store",
        &indexed_e16_m1_vector_memory_program(),
        1040,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        30,
        "cache-backed indexed e16,m1 vector memory should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e16_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e16-m1-load-store",
        &masked_indexed_e16_m1_vector_memory_program(),
        560,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        35,
        "masked indexed e16,m1 vector memory should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e16-m1-load-store",
        &masked_indexed_e16_m1_vector_memory_program(),
        1400,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        35,
        "cache-backed masked indexed e16,m1 vector memory should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e16_m1_data_e8_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e16-data-e8-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e16,m1 data with e8 indices should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e16-data-e8-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e16,m1 data with e8 indices should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e16_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e16-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e16,m1 data with e8 indices should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e16-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e16,m1 data with e8 indices should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e16_m1_data_e32_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e16-data-e32-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e16,m1 data with e32 indices should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e16-data-e32-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e16,m1 data with e32 indices should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e16_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e16-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e16,m1 data with e32 indices should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e16-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e16,m1 data with e32 indices should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e16_m1_data_e64_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e16-data-e64-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        31,
        "mixed-width indexed e16,m1 data with e64 indices should move selected halfword offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e16-data-e64-indices-load-store",
        &mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        1080,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        31,
        "cache-backed mixed-width indexed e16,m1 data with e64 indices should move selected halfword offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e16_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e16-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        600,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        36,
        "masked mixed-width indexed e16,m1 data with e64 indices should preserve inactive compact halfword lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e16-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program(),
        1440,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        36,
        "cache-backed masked mixed-width indexed e16,m1 data with e64 indices should preserve inactive compact halfword lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e32-data-e8-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "mixed-width indexed e32,m1 data with e8 indices should move selected contiguous word offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e32-data-e8-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed mixed-width indexed e32,m1 data with e8 indices should move selected contiguous word offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e32-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked mixed-width indexed e32,m1 data with e8 indices should preserve inactive word lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e32-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked mixed-width indexed e32,m1 data with e8 indices should preserve inactive word lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e32-data-e16-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "mixed-width indexed e32,m1 data with e16 indices should move selected contiguous word offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e32-data-e16-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed mixed-width indexed e32,m1 data with e16 indices should move selected contiguous word offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e32-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e32-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e32-data-e64-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "mixed-width indexed e32,m1 data with e64 indices should move selected contiguous word offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e32-data-e64-indices-load-store",
        &mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed mixed-width indexed e32,m1 data with e64 indices should move selected contiguous word offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e32-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e32-data-e64-indices-load-store",
        &masked_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e32-data-e64-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e32,m1 data with e64 indices should preserve interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e32-data-e64-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e32,m1 data with e64 indices should preserve interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e32-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e32,m1 data with e16 indices should preserve interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e32-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e32,m1 data with e16 indices should preserve interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e32-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e32,m1 data with e8 indices should preserve interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e32-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e32,m1 data with e8 indices should preserve interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e32_m1_memory() {
    const EXPECTED_INDEXED_MEMORY_EXTRA_EXECUTE_CYCLES: u64 = 3;

    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e32-m1-load-store",
        &indexed_e32_m1_vector_memory_program(),
        300,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "indexed e32,m1 vector memory should move selected offsets and preserve skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e32-m1-load-store",
        &indexed_e32_m1_vector_memory_program(),
        820,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed indexed e32,m1 vector memory should move selected offsets and preserve skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );

    let unit_stride_stats = in_order_pipeline_payload_stats(
        "in-order-vector-indexed-unit-stride-latency-baseline",
        &unit_stride_memory_program(true),
    );
    assert_eq!(
        stat_value(
            &direct_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ) - stat_value(
            &unit_stride_stats,
            "sim.cpu0.pipeline.in_order.execute_wait_cycles"
        ),
        EXPECTED_INDEXED_MEMORY_EXTRA_EXECUTE_CYCLES,
        "indexed vector load/store should add the fixed vector LSU execute latency over the unit-stride vector memory baseline\nindexed stats:\n{direct_stats}\nbaseline stats:\n{unit_stride_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-e32-m1-load-store",
        &sparse_indexed_e32_m1_vector_memory_program(),
        340,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "sparse indexed e32,m1 vector memory should preserve two interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-e32-m1-load-store",
        &sparse_indexed_e32_m1_vector_memory_program(),
        900,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed sparse indexed e32,m1 vector memory should preserve two interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_leading_gap_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-leading-gap-e32-m1-load-store",
        &leading_gap_indexed_e32_m1_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "leading-gap indexed e32,m1 vector memory should preserve leading and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-leading-gap-e32-m1-load-store",
        &leading_gap_indexed_e32_m1_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed leading-gap indexed e32,m1 vector memory should preserve leading and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_reversed_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-reversed-e32-m1-load-store",
        &reversed_indexed_e32_m1_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "reversed indexed e32,m1 vector memory should follow non-monotonic offsets and preserve skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-reversed-e32-m1-load-store",
        &reversed_indexed_e32_m1_vector_memory_program(),
        920,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed reversed indexed e32,m1 vector memory should follow non-monotonic offsets and preserve skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e64-data-e8-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "mixed-width indexed e64,m1 data with e8 indices should move selected contiguous 64-bit offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e64-data-e8-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        840,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed mixed-width indexed e64,m1 data with e8 indices should move selected contiguous 64-bit offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e64-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "masked mixed-width indexed e64,m1 data with e8 indices should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e64-data-e8-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed masked mixed-width indexed e64,m1 data with e8 indices should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e64-data-e16-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "mixed-width indexed e64,m1 data with e16 indices should move selected contiguous 64-bit offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e64-data-e16-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        840,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed mixed-width indexed e64,m1 data with e16 indices should move selected contiguous 64-bit offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e64-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "masked mixed-width indexed e64,m1 data with e16 indices should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e64-data-e16-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed masked mixed-width indexed e64,m1 data with e16 indices should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-mixed-e64-data-e32-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        28,
        "mixed-width indexed e64,m1 data with e32 indices should move selected contiguous 64-bit offsets through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-mixed-e64-data-e32-indices-load-store",
        &mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        840,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        28,
        "cache-backed mixed-width indexed e64,m1 data with e32 indices should move selected contiguous 64-bit offsets through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-mixed-e64-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "masked mixed-width indexed e64,m1 data with e32 indices should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-mixed-e64-data-e32-indices-load-store",
        &masked_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed masked mixed-width indexed e64,m1 data with e32 indices should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-e64-m1-load-store",
        &indexed_e64_m1_vector_memory_program(),
        320,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        27,
        "indexed e64,m1 vector memory should move two 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-e64-m1-load-store",
        &indexed_e64_m1_vector_memory_program(),
        760,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        27,
        "cache-backed indexed e64,m1 vector memory should move two 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-e64-m1-load-store",
        &sparse_indexed_e64_m1_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        33,
        "sparse indexed e64,m1 vector memory should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-e64-m1-load-store",
        &sparse_indexed_e64_m1_vector_memory_program(),
        980,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        33,
        "cache-backed sparse indexed e64,m1 vector memory should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e64-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e64,m1 data with e8 indices should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e64-data-e8-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        1020,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e64,m1 data with e8 indices should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e64-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e64,m1 data with e16 indices should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e64-data-e16-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        1020,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e64,m1 data with e16 indices should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_sparse_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-sparse-mixed-e64-data-e32-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        34,
        "sparse mixed-width indexed e64,m1 data with e32 indices should preserve 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-sparse-mixed-e64-data-e32-indices-load-store",
        &sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        1020,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        34,
        "cache-backed sparse mixed-width indexed e64,m1 data with e32 indices should preserve 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e64-m1-load-store",
        &masked_indexed_e64_m1_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        32,
        "masked indexed e64,m1 vector memory should preserve inactive 64-bit lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e64-m1-load-store",
        &masked_indexed_e64_m1_vector_memory_program(),
        1100,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        32,
        "cache-backed masked indexed e64,m1 vector memory should preserve inactive 64-bit lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_vector_indexed_e64_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-e64-m1-load-store",
        &masked_sparse_indexed_e64_m1_vector_memory_program(),
        520,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked sparse indexed e64,m1 vector memory should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-e64-m1-load-store",
        &masked_sparse_indexed_e64_m1_vector_memory_program(),
        1320,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked sparse indexed e64,m1 vector memory should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e64_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e64-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        540,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e64,m1 data with e8 indices should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e64-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program(),
        1360,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e64,m1 data with e8 indices should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e64_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e64-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        540,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e64,m1 data with e16 indices should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e64-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program(),
        1360,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e64,m1 data with e16 indices should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e64_m1_data_e32_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e64-data-e32-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        540,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e64,m1 data with e32 indices should preserve inactive lanes and 64-bit interior gaps through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e64-data-e32-indices-load-store",
        &masked_sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program(),
        1360,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e64,m1 data with e32 indices should preserve inactive lanes and 64-bit interior gaps through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-e32-m1-load-store",
        &masked_indexed_e32_m1_vector_memory_program(),
        380,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked indexed e32,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-e32-m1-load-store",
        &masked_indexed_e32_m1_vector_memory_program(),
        1040,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked indexed e32,m1 vector memory should preserve inactive compact lanes and skipped store bytes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e32_m1_data_e64_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e32-data-e64-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e32-data-e64-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e32,m1 data with e64 indices should preserve inactive compact word lanes and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e32_m1_data_e16_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e32-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e32-data-e16-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e32,m1 data with e16 indices should preserve inactive compact word lanes and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_mixed_width_vector_indexed_e32_m1_data_e8_indices_memory(
) {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-mixed-e32-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        480,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        39,
        "masked sparse mixed-width indexed e32,m1 data with e8 indices should preserve inactive compact word lanes and interior skipped store words through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-mixed-e32-data-e8-indices-load-store",
        &masked_sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        39,
        "cache-backed masked sparse mixed-width indexed e32,m1 data with e8 indices should preserve inactive compact word lanes and interior skipped store words through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_sparse_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-sparse-e32-m1-load-store",
        &masked_sparse_indexed_e32_m1_vector_memory_program(),
        420,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked sparse indexed e32,m1 vector memory should preserve interior gaps and the inactive sparse lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-sparse-e32-m1-load-store",
        &masked_sparse_indexed_e32_m1_vector_memory_program(),
        1120,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked sparse indexed e32,m1 vector memory should preserve interior gaps and the inactive sparse lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_leading_gap_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-leading-gap-e32-m1-load-store",
        &masked_leading_gap_indexed_e32_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked leading-gap indexed e32,m1 vector memory should preserve the leading gap, interior gap, and inactive sparse lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-leading-gap-e32-m1-load-store",
        &masked_leading_gap_indexed_e32_m1_vector_memory_program(),
        1140,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked leading-gap indexed e32,m1 vector memory should preserve the leading gap, interior gap, and inactive sparse lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_trailing_active_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-trailing-active-e32-m1-load-store",
        &masked_trailing_active_indexed_e32_m1_vector_memory_program(),
        440,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked trailing-active indexed e32,m1 vector memory should suppress the first lane while loading and storing the later active indexed lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-trailing-active-e32-m1-load-store",
        &masked_trailing_active_indexed_e32_m1_vector_memory_program(),
        1140,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked trailing-active indexed e32,m1 vector memory should suppress the first lane while loading and storing the later active indexed lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_reversed_vector_indexed_e32_m1_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-indexed-masked-reversed-e32-m1-load-store",
        &masked_reversed_indexed_e32_m1_vector_memory_program(),
        460,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        38,
        "masked reversed indexed e32,m1 vector memory should keep the inactive lower-offset lane untouched while storing only the active high-offset lane through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-indexed-masked-reversed-e32-m1-load-store",
        &masked_reversed_indexed_e32_m1_vector_memory_program(),
        1160,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        38,
        "cache-backed masked reversed indexed e32,m1 vector memory should keep the inactive lower-offset lane untouched while storing only the active high-offset lane through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_full_lmul4_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-masked-full-lmul4-load-store",
        &masked_unit_stride_lmul4_vector_memory_program(),
        620,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        114,
        "masked LMUL4 unit-stride vector memory should move active lanes and preserve inactive lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-masked-full-lmul4-load-store",
        &masked_unit_stride_lmul4_vector_memory_program(),
        1500,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        114,
        "cache-backed masked LMUL4 unit-stride vector memory should move active lanes and preserve inactive lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_full_lmul8_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-masked-full-lmul8-load-store",
        &masked_unit_stride_lmul8_vector_memory_program(),
        1200,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        210,
        "masked LMUL8 unit-stride vector memory should move active lanes and preserve inactive lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-masked-full-lmul8-load-store",
        &masked_unit_stride_lmul8_vector_memory_program(),
        3600,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        210,
        "cache-backed masked LMUL8 unit-stride vector memory should move active lanes and preserve inactive lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_lmul2_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-masked-lmul2-load-store",
        &masked_unit_stride_lmul2_vector_memory_program(),
        360,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        66,
        "masked LMUL2 unit-stride vector memory should retire through the direct-memory success ecall\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-masked-lmul2-load-store",
        &masked_unit_stride_lmul2_vector_memory_program(),
        700,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        66,
        "cache-backed masked LMUL2 unit-stride vector memory should retire through the success ecall\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_e16_lmul2_register_group_memory() {
    let direct_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-masked-e16-lmul2-load-store",
        &masked_unit_stride_e16_lmul2_vector_memory_program(),
        620,
    );

    assert_eq!(
        stat_value(&direct_stats, "sim.cpu0.instructions.committed"),
        114,
        "masked e16/m2 unit-stride vector memory should preserve inactive halfword lanes and move active halfword lanes through the direct-memory top-level run path\nstats:\n{direct_stats}"
    );

    let cache_stats = in_order_pipeline_payload_stats_with_default_memory_system(
        "in-order-cache-vector-unit-stride-masked-e16-lmul2-load-store",
        &masked_unit_stride_e16_lmul2_vector_memory_program(),
        1000,
    );

    assert_eq!(
        stat_value(&cache_stats, "sim.cpu0.instructions.committed"),
        114,
        "cache-backed masked e16/m2 unit-stride vector memory should preserve inactive halfword lanes and move active halfword lanes through the top-level run path\nstats:\n{cache_stats}"
    );
}

#[test]
fn rem6_run_rejects_misaligned_vector_unit_stride_full_lmul2_register_group_memory() {
    assert_in_order_pipeline_payload_fails_with_error(
        "in-order-vector-unit-stride-full-lmul2-misaligned-load-store",
        &unit_stride_lmul2_vector_memory_program_with_data_offset(8, 196),
        "RISC-V PMA denied misaligned Read access at 0x800000c4 with 32 byte(s)",
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_zero_vl_vector_memory_as_no_data_access() {
    let stats = in_order_pipeline_payload_stats(
        "in-order-vector-unit-stride-zero-vl-no-data-access",
        &riscv64_program(&[
            i_type(64, 0, 0b000, 10, 0x13), // addi x10, x0, data
            i_type(0, 0, 0b000, 11, 0x13),  // addi x11, x0, 0
            vsetvli_type(0xd0, 11, 5),      // vsetvli x5, x11, e32, m1, ta, ma
            0x0205_6087,                    // vle32.v v1, (x10)
            0x0205_60a7,                    // vse32.v v1, (x10)
            0x0000_0073,                    // ecall
        ]),
    );

    assert_eq!(
        stat_value(&stats, "sim.cpu0.instructions.committed"),
        6,
        "zero-vl vector memory should retire through the success ecall\nstats:\n{stats}"
    );
    assert_eq!(
        stat_value(&stats, "sim.cpu0.pipeline.in_order.data_wait_cycles"),
        0,
        "zero-vl vector memory should not issue a data access\nstats:\n{stats}"
    );
    assert_eq!(
        stat_value(&stats, "sim.cpu0.pipeline.in_order.execute_wait_cycles"),
        3,
        "zero-vl vector memory still pays the fixed vector LSU execute latency\nstats:\n{stats}"
    );
}

#[test]
fn rem6_run_rejects_unsupported_vector_memory_slice() {
    for (name, vtype, instruction) in [
        (
            "in-order-vector-memory-rejects-e32-mf2",
            0xd7,
            0x0205_6087, // vle32.v v1, (x10)
        ),
        (
            "in-order-vector-memory-rejects-misaligned-e32-m2",
            0xd1,
            0x0205_6087, // vle32.v v1, (x10)
        ),
    ] {
        assert_in_order_pipeline_payload_rejected(
            name,
            &vector_memory_single_access_program(vtype, instruction),
        );
    }
}

#[test]
fn rem6_run_stats_emit_checker_cpu_counts_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("checker-cpu-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "40",
            "--stats-format",
            "json",
            "--execute",
            "--checker-cpu",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let checker = json
        .pointer("/cores/0/checker")
        .unwrap_or_else(|| panic!("missing checker JSON summary: {json}"));
    assert_eq!(
        checker
            .pointer("/checked_instructions")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        checker.pointer("/mismatches").and_then(Value::as_u64),
        Some(0)
    );
    assert_eq!(
        checker.pointer("/execution_mode").and_then(Value::as_str),
        Some("functional")
    );
    assert_stat(
        &stdout,
        "sim.cpu0.checker.checked_instructions",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.checker.mismatches",
        "Count",
        0,
        "monotonic",
    );
    for (mode, checked_instructions) in [("functional", 2), ("timing", 0), ("detailed", 0)] {
        assert_eq!(
            json_stat_value(
                &json,
                &format!("sim.cpu0.checker.execution_mode.{mode}.checked_instructions")
            ),
            checked_instructions
        );
        assert_eq!(
            json_stat_value(
                &json,
                &format!("sim.cpu0.checker.execution_mode.{mode}.mismatches")
            ),
            0
        );
    }
}

fn in_order_pipeline_latency_stats(name: &str, words: &[u32]) -> String {
    let program = riscv64_program(words);
    in_order_pipeline_payload_stats(name, &program)
}

fn in_order_pipeline_payload_stats(name: &str, program: &[u8]) -> String {
    in_order_pipeline_payload_stats_with_max_tick(name, program, 80)
}

fn in_order_pipeline_payload_stats_with_max_tick(
    name: &str,
    program: &[u8],
    max_tick: u64,
) -> String {
    let pipeline_stage_slack_ticks = u64::try_from(program.len()).unwrap();
    in_order_pipeline_payload_stats_with_optional_memory_system(
        name,
        program,
        max_tick,
        pipeline_stage_slack_ticks,
        Some("direct"),
    )
}

fn in_order_pipeline_payload_stats_with_default_memory_system(
    name: &str,
    program: &[u8],
    max_tick: u64,
) -> String {
    let pipeline_stage_slack_ticks = u64::try_from(program.len()).unwrap();
    in_order_pipeline_payload_stats_with_optional_memory_system(
        name,
        program,
        max_tick,
        pipeline_stage_slack_ticks,
        None,
    )
}

fn in_order_pipeline_payload_stats_with_optional_memory_system(
    name: &str,
    program: &[u8],
    base_max_tick: u64,
    pipeline_stage_slack_ticks: u64,
    memory_system: Option<&str>,
) -> String {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);
    let max_tick = base_max_tick
        .checked_add(pipeline_stage_slack_ticks)
        .expect("pipeline timing test max tick should not overflow")
        .to_string();

    let mut command = Command::new(env!("CARGO_BIN_EXE_rem6"));
    command.args([
        "run",
        "--isa",
        "riscv",
        "--binary",
        path.to_str().unwrap(),
        "--max-tick",
        max_tick.as_str(),
        "--stats-format",
        "json",
        "--execute",
    ]);
    if let Some(memory_system) = memory_system {
        command.args(["--memory-system", memory_system]);
    }
    let output = command.output().unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn simulation_trap(stats: &str) -> Option<String> {
    let payload: Value = serde_json::from_str(stats).unwrap();
    payload
        .pointer("/simulation/trap")
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn assert_in_order_pipeline_payload_rejected(name: &str, program: &[u8]) {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "{name} should report an executed illegal-instruction trap, not a CLI error\nstderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let payload: Value = serde_json::from_str(&stdout).unwrap();
    let trap = payload
        .pointer("/simulation/trap")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    assert_eq!(
        trap, "illegal_instruction",
        "{name} should be rejected outside the supported unit-stride vector memory slice"
    );
}

fn assert_in_order_pipeline_payload_fails_with_error(
    name: &str,
    program: &[u8],
    expected_error: &str,
) {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        !output.status.success(),
        "{name} should fail with a CLI execution error"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(expected_error),
        "{name} stderr should contain {expected_error:?}\nstderr: {stderr}"
    );
}

fn unit_stride_memory_program(use_vector_memory: bool) -> Vec<u8> {
    let memory_move = if use_vector_memory {
        [
            0x0205_6087, // vle32.v v1, (x10)
            0x0208_60a7, // vse32.v v1, (x16)
        ]
    } else {
        [
            i_type(0, 10, 0b011, 6, 0x03), // ld x6, 0(x10)
            s_type(0, 6, 16, 0b011),       // sd x6, 0(x16)
        ]
    };
    unit_stride_memory_program_with_move(0xd0, 2, memory_move)
}

fn unit_stride_vector_memory_program(vtype: u32, avl: i32, load: u32, store: u32) -> Vec<u8> {
    unit_stride_memory_program_with_move(vtype, avl, [load, store])
}

fn fault_only_first_later_line_fault_program() -> Vec<u8> {
    const DEST_OFFSET_BYTES: i32 = 256;
    const INITIAL_VECTOR_OFFSET_BYTES: i32 = 264;
    const EXPECTED_LANE1_OFFSET_BYTES: i32 = 272;
    const FAULT_ONLY_SOURCE_OFFSET_BYTES: usize = 0xffc;

    let words = [
        u_type(0x1000, 10, 0x17),                           // auipc x10, 0x1
        i_type(-4, 10, 0b000, 10, 0x13), // addi x10, x10, -4; source at end of mapped page
        u_type(0, 16, 0x17),             // auipc x16, 0
        i_type(DEST_OFFSET_BYTES - 8, 16, 0b000, 16, 0x13), // addi x16, x16, dest
        u_type(0, 17, 0x17),             // auipc x17, 0
        i_type(INITIAL_VECTOR_OFFSET_BYTES - 16, 17, 0b000, 17, 0x13), // addi x17, x17, initial vector
        u_type(0, 18, 0x17),                                           // auipc x18, 0
        i_type(EXPECTED_LANE1_OFFSET_BYTES - 24, 18, 0b000, 18, 0x13), // addi x18, x18, expected lane 1
        i_type(2, 0, 0b000, 11, 0x13),                                 // addi x11, x0, 2
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 17, 1), // vle32.v v1, (x17)
        vector_unit_stride_fault_only_load_type(true, 0b110, 10, 1), // vle32ff.v v1, (x10)
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse32.v v1, (x16)
        i_type(0, 16, 0b010, 12, 0x03), // lw x12, 0(x16)
        i_type(0, 10, 0b010, 13, 0x03), // lw x13, 0(x10)
        b_type(20, 13, 12, 0b001), // bne x12, x13, fail
        i_type(4, 16, 0b010, 14, 0x03), // lw x14, 4(x16)
        i_type(0, 18, 0b010, 15, 0x03), // lw x15, 0(x18)
        b_type(8, 15, 14, 0b001),  // bne x14, x15, fail
        0x0000_0073,               // ecall
        0x0000_0000,               // fail: invalid instruction
    ];

    let mut program = riscv64_program(&words);
    while program.len() < DEST_OFFSET_BYTES as usize {
        program.push(0);
    }
    program.extend_from_slice(&0x1111_1111_u32.to_le_bytes());
    program.extend_from_slice(&0x2222_2222_u32.to_le_bytes());
    program.extend_from_slice(&0xaaaa_5555_u32.to_le_bytes());
    program.extend_from_slice(&0xbbbb_6666_u32.to_le_bytes());
    program.extend_from_slice(&0x2222_2222_u32.to_le_bytes());
    while program.len() < FAULT_ONLY_SOURCE_OFFSET_BYTES {
        program.push(0);
    }
    program.extend_from_slice(&0xcafe_babe_u32.to_le_bytes());
    program
}

fn masked_fault_only_first_inactive_later_line_program() -> Vec<u8> {
    const MASK_OFFSET_BYTES: i32 = 256;
    const DEST_OFFSET_BYTES: i32 = 264;
    const INITIAL_VECTOR_OFFSET_BYTES: i32 = 272;
    const FAULT_ONLY_SOURCE_OFFSET_BYTES: usize = 0xffc;

    let words = [
        u_type(0x1000, 10, 0x17),                            // auipc x10, 0x1
        i_type(-4, 10, 0b000, 10, 0x13), // addi x10, x10, -4; source at end of mapped page
        u_type(0, 12, 0x17),             // auipc x12, 0
        i_type(MASK_OFFSET_BYTES - 8, 12, 0b000, 12, 0x13), // addi x12, x12, mask bits
        u_type(0, 16, 0x17),             // auipc x16, 0
        i_type(DEST_OFFSET_BYTES - 16, 16, 0b000, 16, 0x13), // addi x16, x16, dest
        u_type(0, 17, 0x17),             // auipc x17, 0
        i_type(INITIAL_VECTOR_OFFSET_BYTES - 24, 17, 0b000, 17, 0x13), // addi x17, x17, initial vector
        i_type(2, 0, 0b000, 11, 0x13),                                 // addi x11, x0, 2
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 0), // vle32.v v0, (x12)
        vector_unit_stride_load_type(true, 0b110, 17, 1), // vle32.v v1, (x17)
        vector_unit_stride_fault_only_load_type(false, 0b110, 10, 1), // vle32ff.v v1, (x10), v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse32.v v1, (x16)
        i_type(0, 16, 0b010, 20, 0x03), // lw x20, 0(x16)
        i_type(0, 10, 0b010, 21, 0x03), // lw x21, 0(x10)
        b_type(16, 21, 20, 0b001), // bne x20, x21, fail
        i_type(4, 16, 0b010, 22, 0x03), // lw x22, 4(x16)
        i_type(4, 17, 0b010, 23, 0x03), // lw x23, 4(x17)
        b_type(8, 23, 22, 0b001),  // bne x22, x23, fail
        0x0000_0073,               // ecall
        0x0000_0000,               // fail: invalid instruction
    ];

    let mut program = riscv64_program(&words);
    while program.len() < MASK_OFFSET_BYTES as usize {
        program.push(0);
    }
    program.extend_from_slice(&0x0000_0001_u32.to_le_bytes());
    program.extend_from_slice(&0x0000_0000_u32.to_le_bytes());
    program.extend_from_slice(&0x1111_1111_u32.to_le_bytes());
    program.extend_from_slice(&0x2222_2222_u32.to_le_bytes());
    program.extend_from_slice(&0xaaaa_5555_u32.to_le_bytes());
    program.extend_from_slice(&0xbbbb_6666_u32.to_le_bytes());
    while program.len() < FAULT_ONLY_SOURCE_OFFSET_BYTES {
        program.push(0);
    }
    program.extend_from_slice(&0xcafe_babe_u32.to_le_bytes());
    program
}

fn masked_fault_only_first_active_later_line_program() -> Vec<u8> {
    const MASK_OFFSET_BYTES: i32 = 256;
    const DEST_OFFSET_BYTES: i32 = 264;
    const INITIAL_VECTOR_OFFSET_BYTES: i32 = 272;
    const EXPECTED_LANE1_OFFSET_BYTES: i32 = 280;
    const FAULT_ONLY_SOURCE_OFFSET_BYTES: usize = 0xffc;

    let words = [
        u_type(0x1000, 10, 0x17),                            // auipc x10, 0x1
        i_type(-4, 10, 0b000, 10, 0x13), // addi x10, x10, -4; source at end of mapped page
        u_type(0, 12, 0x17),             // auipc x12, 0
        i_type(MASK_OFFSET_BYTES - 8, 12, 0b000, 12, 0x13), // addi x12, x12, mask bits
        u_type(0, 16, 0x17),             // auipc x16, 0
        i_type(DEST_OFFSET_BYTES - 16, 16, 0b000, 16, 0x13), // addi x16, x16, dest
        u_type(0, 17, 0x17),             // auipc x17, 0
        i_type(INITIAL_VECTOR_OFFSET_BYTES - 24, 17, 0b000, 17, 0x13), // addi x17, x17, initial vector
        u_type(0, 18, 0x17),                                           // auipc x18, 0
        i_type(EXPECTED_LANE1_OFFSET_BYTES - 32, 18, 0b000, 18, 0x13), // addi x18, x18, expected lane 1
        i_type(2, 0, 0b000, 11, 0x13),                                 // addi x11, x0, 2
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 0), // vle32.v v0, (x12)
        vector_unit_stride_load_type(true, 0b110, 17, 1), // vle32.v v1, (x17)
        vector_unit_stride_fault_only_load_type(false, 0b110, 10, 1), // vle32ff.v v1, (x10), v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse32.v v1, (x16)
        i_type(0, 16, 0b010, 20, 0x03), // lw x20, 0(x16)
        i_type(0, 10, 0b010, 21, 0x03), // lw x21, 0(x10)
        b_type(16, 21, 20, 0b001), // bne x20, x21, fail
        i_type(4, 16, 0b010, 22, 0x03), // lw x22, 4(x16)
        i_type(0, 18, 0b010, 23, 0x03), // lw x23, 0(x18)
        b_type(8, 23, 22, 0b001),  // bne x22, x23, fail
        0x0000_0073,               // ecall
        0x0000_0000,               // fail: invalid instruction
    ];

    let mut program = riscv64_program(&words);
    while program.len() < MASK_OFFSET_BYTES as usize {
        program.push(0);
    }
    program.extend_from_slice(&0x0000_0003_u32.to_le_bytes());
    program.extend_from_slice(&0x0000_0000_u32.to_le_bytes());
    program.extend_from_slice(&0x1111_1111_u32.to_le_bytes());
    program.extend_from_slice(&0x2222_2222_u32.to_le_bytes());
    program.extend_from_slice(&0xaaaa_5555_u32.to_le_bytes());
    program.extend_from_slice(&0xbbbb_6666_u32.to_le_bytes());
    program.extend_from_slice(&0x2222_2222_u32.to_le_bytes());
    while program.len() < FAULT_ONLY_SOURCE_OFFSET_BYTES {
        program.push(0);
    }
    program.extend_from_slice(&0xcafe_babe_u32.to_le_bytes());
    program
}

fn unit_stride_memory_program_with_move(vtype: u32, avl: i32, memory_move: [u32; 2]) -> Vec<u8> {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),             // auipc x10, 0
        i_type(64, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(8, 10, 0b000, 16, 0x13),  // addi x16, x10, 8
        i_type(avl, 0, 0b000, 11, 0x13), // addi x11, x0, avl
        vsetvli_type(vtype, 11, 5),      // vsetvli x5, x11, e*, m1, ta, ma
        memory_move[0],
        memory_move[1],
        i_type(0, 16, 0b010, 12, 0x03), // lw x12, 0(x16)
        i_type(4, 16, 0b010, 13, 0x03), // lw x13, 4(x16)
        i_type(0, 10, 0b010, 14, 0x03), // lw x14, 0(x10)
        i_type(4, 10, 0b010, 15, 0x03), // lw x15, 4(x10)
        b_type(12, 14, 12, 0b001),      // bne x12, x14, fail
        b_type(8, 15, 13, 0b001),       // bne x13, x15, fail
        0x0000_0073,                    // ecall
        0x0000_0000,                    // fail: invalid instruction
        0x0000_0000,                    // padding to keep data doubleword-aligned
    ]);
    program.extend_from_slice(&[
        0x44, 0x33, 0x22, 0x11, 0x88, 0x77, 0x66, 0x55, 0, 0, 0, 0, 0, 0, 0, 0,
    ]);
    program
}

fn vector_memory_single_access_program(vtype: u32, instruction: u32) -> Vec<u8> {
    let mut program = riscv64_program(&[
        u_type(0, 10, 0x17),             // auipc x10, 0
        i_type(32, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(2, 0, 0b000, 11, 0x13),   // addi x11, x0, 2
        vsetvli_type(vtype, 11, 5),
        instruction,
        0x0000_0073, // ecall
        0x0000_0000, // padding to keep data doubleword-aligned
        0x0000_0000, // padding
    ]);
    program.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0x11, 0x22, 0x33, 0x44]);
    program
}

fn unit_stride_lmul2_vector_memory_program(data_words: usize) -> Vec<u8> {
    unit_stride_lmul2_vector_memory_program_with_load(
        data_words,
        192,
        vector_unit_stride_load_type(true, 0b110, 10, 2),
    )
}

fn unit_stride_e16_lmul2_vector_memory_program() -> Vec<u8> {
    unit_stride_lmul2_vector_memory_program_with_shape(
        16,
        2,
        256,
        0xc9,
        vector_unit_stride_load_type(true, 0b101, 10, 2),
        vector_unit_stride_store_type(true, 0b101, 16, 2),
    )
}

fn unit_stride_e16_lmul4_vector_memory_program() -> Vec<u8> {
    unit_stride_register_group_vector_memory_program_with_shape(
        32,
        2,
        512,
        0xca,
        vector_unit_stride_load_type(true, 0b101, 10, 4),
        vector_unit_stride_store_type(true, 0b101, 16, 4),
    )
}

fn fault_only_lmul2_vector_memory_program(data_words: usize) -> Vec<u8> {
    unit_stride_lmul2_vector_memory_program_with_load(
        data_words,
        192,
        vector_unit_stride_fault_only_load_type(true, 0b110, 10, 2),
    )
}

fn unit_stride_lmul2_vector_memory_program_with_data_offset(
    data_words: usize,
    data_offset_bytes: i32,
) -> Vec<u8> {
    unit_stride_lmul2_vector_memory_program_with_load(
        data_words,
        data_offset_bytes,
        vector_unit_stride_load_type(true, 0b110, 10, 2),
    )
}

fn unit_stride_lmul2_vector_memory_program_with_load(
    data_words: usize,
    data_offset_bytes: i32,
    load: u32,
) -> Vec<u8> {
    unit_stride_lmul2_vector_memory_program_with_shape(
        data_words,
        4,
        data_offset_bytes,
        0xd1,
        load,
        0x0208_6127, // vse32.v v2, (x16)
    )
}

fn unit_stride_lmul2_vector_memory_program_with_shape(
    data_elements: usize,
    element_bytes: usize,
    data_offset_bytes: i32,
    vtype: u32,
    load: u32,
    store: u32,
) -> Vec<u8> {
    unit_stride_register_group_vector_memory_program_with_shape(
        data_elements,
        element_bytes,
        data_offset_bytes,
        vtype,
        load,
        store,
    )
}

fn unit_stride_register_group_vector_memory_program_with_shape(
    data_elements: usize,
    element_bytes: usize,
    data_offset_bytes: i32,
    vtype: u32,
    load: u32,
    store: u32,
) -> Vec<u8> {
    assert!((1..=32).contains(&data_elements));
    assert!(data_offset_bytes > 0 && data_offset_bytes % 4 == 0);
    assert!(matches!(element_bytes, 1 | 2 | 4 | 8));
    assert!(data_elements * element_bytes <= 128);
    let data_bytes = (data_elements * element_bytes) as i32;
    let load_fun3 = match element_bytes {
        1 => 0b000,
        2 => 0b001,
        4 => 0b010,
        8 => 0b011,
        _ => unreachable!("validated element width"),
    };
    let fail_instruction_index = 7 + data_elements as i32 * 3 + 1;

    let mut words = vec![
        u_type(0, 10, 0x17),                              // auipc x10, 0
        i_type(data_offset_bytes, 10, 0b000, 10, 0x13),   // addi x10, x10, data
        i_type(data_bytes, 10, 0b000, 16, 0x13),          // addi x16, x10, dest
        i_type(data_elements as i32, 0, 0b000, 11, 0x13), // addi x11, x0, vl
        vsetvli_type(vtype, 11, 5),
        load,
        store,
    ];

    for element_index in 0..data_elements {
        let offset = (element_index * element_bytes) as i32;
        words.push(i_type(offset, 16, load_fun3, 12, 0x03)); // load x12, dest+i
        words.push(i_type(offset, 10, load_fun3, 13, 0x03)); // load x13, source+i
        let branch_index = words.len() as i32;
        let fail_offset = (fail_instruction_index - branch_index) * 4;
        words.push(b_type(fail_offset, 13, 12, 0b001)); // bne x12, x13, fail
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    while words.len() * 4 < data_offset_bytes as usize {
        words.push(0);
    }

    let mut program = riscv64_program(&words);
    for element_index in 0..data_elements {
        let value =
            0x0102_0304_0506_0708_u64.wrapping_add((element_index as u64) * 0x0101_0101_0101_0101);
        program.extend_from_slice(&value.to_le_bytes()[..element_bytes]);
    }
    program.extend(std::iter::repeat_n(0, data_elements * element_bytes));
    program
}

fn unit_stride_lmul4_vector_memory_program() -> Vec<u8> {
    unit_stride_lmul4_vector_memory_program_with_load(vector_unit_stride_load_type(
        true, 0b110, 10, 4,
    ))
}

fn fault_only_lmul4_vector_memory_program() -> Vec<u8> {
    unit_stride_lmul4_vector_memory_program_with_load(vector_unit_stride_fault_only_load_type(
        true, 0b110, 10, 4,
    ))
}

fn unit_stride_lmul4_vector_memory_program_with_load(load: u32) -> Vec<u8> {
    const DATA_WORDS: usize = 16;
    const DATA_OFFSET_BYTES: i32 = 256;
    const VECTOR_BYTES: i32 = (DATA_WORDS * 4) as i32;

    let fail_instruction_index = 7 + DATA_WORDS as i32 * 3 + 1;
    let mut words = vec![
        u_type(0, 10, 0x17),                            // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(VECTOR_BYTES, 10, 0b000, 16, 0x13),      // addi x16, x10, dest
        i_type(DATA_WORDS as i32, 0, 0b000, 11, 0x13),  // addi x11, x0, vl
        vsetvli_type(0xd2, 11, 5),                      // vsetvli x5, x11, e32, m4, ta, ma
        load,
        vector_unit_stride_store_type(true, 0b110, 16, 4), // vse32.v v4, (x16)
    ];

    for word_index in 0..DATA_WORDS {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 12, 0x03)); // lw x12, dest+i
        words.push(i_type(offset, 10, 0b010, 13, 0x03)); // lw x13, source+i
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            13,
            12,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mut program = riscv64_program(&words);
    for word_index in 0..DATA_WORDS {
        let value = 0x0102_0304_u32.wrapping_add((word_index as u32) * 0x1111_1111);
        program.extend_from_slice(&value.to_le_bytes());
    }
    program.extend(std::iter::repeat_n(0, DATA_WORDS * 4));
    program
}

fn unit_stride_lmul8_vector_memory_program() -> Vec<u8> {
    unit_stride_lmul8_vector_memory_program_with_load(vector_unit_stride_load_type(
        true, 0b110, 10, 8,
    ))
}

fn fault_only_lmul8_vector_memory_program() -> Vec<u8> {
    unit_stride_lmul8_vector_memory_program_with_load(vector_unit_stride_fault_only_load_type(
        true, 0b110, 10, 8,
    ))
}

fn unit_stride_lmul8_vector_memory_program_with_load(load: u32) -> Vec<u8> {
    const DATA_WORDS: usize = 32;
    const DATA_OFFSET_BYTES: i32 = 512;
    const VECTOR_BYTES: i32 = (DATA_WORDS * 4) as i32;

    let fail_instruction_index = 7 + DATA_WORDS as i32 * 3 + 1;
    let mut words = vec![
        u_type(0, 10, 0x17),                            // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(VECTOR_BYTES, 10, 0b000, 16, 0x13),      // addi x16, x10, dest
        i_type(DATA_WORDS as i32, 0, 0b000, 11, 0x13),  // addi x11, x0, vl
        vsetvli_type(0xd3, 11, 5),                      // vsetvli x5, x11, e32, m8, ta, ma
        load,
        vector_unit_stride_store_type(true, 0b110, 16, 8), // vse32.v v8, (x16)
    ];

    for word_index in 0..DATA_WORDS {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 12, 0x03)); // lw x12, dest+i
        words.push(i_type(offset, 10, 0b010, 13, 0x03)); // lw x13, source+i
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            13,
            12,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mut program = riscv64_program(&words);
    for word_index in 0..DATA_WORDS {
        let value = 0x0102_0304_u32.wrapping_add((word_index as u32).wrapping_mul(0x1111_1111));
        program.extend_from_slice(&value.to_le_bytes());
    }
    program.extend(std::iter::repeat_n(0, DATA_WORDS * 4));
    program
}

fn strided_e8_m1_stride15_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const SOURCE_OFFSET_BYTES: i32 = 0;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 16;
    const STORE_RESULT_OFFSET_BYTES: i32 = 32;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 48;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 64;
    const STRIDE_BYTES: i32 = 15;

    let fail_instruction_index = 29;
    let words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),    // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),                 // addi x21, x0, stride
        vsetvli_type(0xc0, 11, 5),                                // vsetvli x5, x11, e8, m1, ta, ma
        vector_strided_load_type(true, 0b000, 14, 21, 1),         // vlse8.v v1, (x14), x21
        vector_unit_stride_store_type(true, 0b000, 15, 1),        // vse8.v v1, (x15)
        vector_strided_store_type(true, 0b000, 16, 21, 1),        // vsse8.v v1, (x16), x21
        i_type(0, 15, 0b010, 17, 0x03),                           // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03),                           // lw x18, expected load word 0
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_22a1_u32, 0x3333_4444, 0x5555_6666, 0xb177_8888];

    let mut program = riscv64_program(&program_words);
    for word in source_span
        .into_iter()
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e8_m1_stride15_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = 8;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;
    const STRIDE_BYTES: i32 = 15;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),           // addi x12, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),        // addi x13, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),    // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),                 // addi x21, x0, stride
        vsetvli_type(0xc0, 11, 5),                                // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 8),         // vle8.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                        // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b000, 13, 1),         // vle8.v v1, (x13)
        vector_strided_load_type(false, 0b000, 14, 21, 1),        // vlse8.v v1, (x14), x21, v0.t
        vector_unit_stride_store_type(true, 0b000, 15, 1),        // vse8.v v1, (x15)
        vector_strided_store_type(false, 0b000, 16, 21, 1),       // vsse8.v v1, (x16), x21, v0.t
        i_type(0, 15, 0b010, 17, 0x03),                           // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03),                           // lw x18, expected load word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mask = [0xeeee_0100_u32, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e8_m1_stride7_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const SOURCE_OFFSET_BYTES: i32 = 0;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 16;
    const STORE_RESULT_OFFSET_BYTES: i32 = 32;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 48;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 64;
    const STRIDE_BYTES: i32 = 7;

    let fail_instruction_index = 29;
    let words = vec![
        u_type(0, 10, 0x17),
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),
        vsetvli_type(0xc0, 11, 5),
        vector_strided_load_type(true, 0b000, 14, 21, 1),
        vector_unit_stride_store_type(true, 0b000, 15, 1),
        vector_strided_store_type(true, 0b000, 16, 21, 1),
        i_type(0, 15, 0b010, 17, 0x03),
        i_type(0, 19, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03),
        i_type(0, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03),
        i_type(4, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03),
        i_type(8, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03),
        i_type(12, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        0x0000_0073,
        0x0000_0000,
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source_span = [0x5151_51a1_u32, 0xb161_6161, 0x7171_7171, 0x8181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_22a1_u32, 0xb133_4444, 0x5555_6666, 0x7777_8888];

    let mut program = riscv64_program(&program_words);
    for word in source_span
        .into_iter()
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e8_m1_stride7_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = 8;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;
    const STRIDE_BYTES: i32 = 7;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),
        vsetvli_type(0xc0, 11, 5),
        vector_unit_stride_load_type(true, 0b000, 12, 8),
        vector_vi_type(0b011000, 8, 0, 0),
        vector_unit_stride_load_type(true, 0b000, 13, 1),
        vector_strided_load_type(false, 0b000, 14, 21, 1),
        vector_unit_stride_store_type(true, 0b000, 15, 1),
        vector_strided_store_type(false, 0b000, 16, 21, 1),
        i_type(0, 15, 0b010, 17, 0x03),
        i_type(0, 19, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03),
        i_type(0, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03),
        i_type(4, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03),
        i_type(8, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03),
        i_type(12, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073,
        0x0000_0000,
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mask = [0xeeee_0100_u32, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee];
    let source_span = [0x5151_51a1_u32, 0xb161_6161, 0x7171_7171, 0x8181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e8_m1_stride3_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const SOURCE_OFFSET_BYTES: i32 = 0;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 16;
    const STORE_RESULT_OFFSET_BYTES: i32 = 32;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 48;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 64;
    const STRIDE_BYTES: i32 = 3;

    let fail_instruction_index = 29;
    let words = vec![
        u_type(0, 10, 0x17),
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),
        vsetvli_type(0xc0, 11, 5),
        vector_strided_load_type(true, 0b000, 14, 21, 1),
        vector_unit_stride_store_type(true, 0b000, 15, 1),
        vector_strided_store_type(true, 0b000, 16, 21, 1),
        i_type(0, 15, 0b010, 17, 0x03),
        i_type(0, 19, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03),
        i_type(0, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03),
        i_type(4, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03),
        i_type(8, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03),
        i_type(12, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        0x0000_0073,
        0x0000_0000,
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source_span = [0xb151_51a1_u32, 0x6161_6161, 0x7171_7171, 0x8181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xb111_22a1_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];

    let mut program = riscv64_program(&program_words);
    for word in source_span
        .into_iter()
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e8_m1_stride3_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = 8;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;
    const STRIDE_BYTES: i32 = 3;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),
        vsetvli_type(0xc0, 11, 5),
        vector_unit_stride_load_type(true, 0b000, 12, 8),
        vector_vi_type(0b011000, 8, 0, 0),
        vector_unit_stride_load_type(true, 0b000, 13, 1),
        vector_strided_load_type(false, 0b000, 14, 21, 1),
        vector_unit_stride_store_type(true, 0b000, 15, 1),
        vector_strided_store_type(false, 0b000, 16, 21, 1),
        i_type(0, 15, 0b010, 17, 0x03),
        i_type(0, 19, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03),
        i_type(0, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03),
        i_type(4, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03),
        i_type(8, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03),
        i_type(12, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073,
        0x0000_0000,
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mask = [0xeeee_0100_u32, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee];
    let source_span = [0xb151_51a1_u32, 0x6161_6161, 0x7171_7171, 0x8181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e16_m1_stride14_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const SOURCE_OFFSET_BYTES: i32 = 0;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 16;
    const STORE_RESULT_OFFSET_BYTES: i32 = 32;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 48;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 64;
    const STRIDE_BYTES: i32 = 14;

    let fail_instruction_index = 29;
    let words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),    // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),                 // addi x21, x0, stride
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_strided_load_type(true, 0b101, 14, 21, 1), // vlse16.v v1, (x14), x21
        vector_unit_stride_store_type(true, 0b101, 15, 1), // vse16.v v1, (x15)
        vector_strided_store_type(true, 0b101, 16, 21, 1), // vsse16.v v1, (x16), x21
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xb1b2_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_a1a2_u32, 0x3333_4444, 0x5555_6666, 0xb1b2_8888];

    let mut program = riscv64_program(&program_words);
    for word in source_span
        .into_iter()
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e16_m1_stride14_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = 8;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;
    const STRIDE_BYTES: i32 = 14;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),           // addi x12, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),        // addi x13, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),    // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),                 // addi x21, x0, stride
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 8), // vle16.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b101, 13, 1), // vle16.v v1, (x13)
        vector_strided_load_type(false, 0b101, 14, 21, 1), // vlse16.v v1, (x14), x21, v0.t
        vector_unit_stride_store_type(true, 0b101, 15, 1), // vse16.v v1, (x15)
        vector_strided_store_type(false, 0b101, 16, 21, 1), // vsse16.v v1, (x16), x21, v0.t
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mask = [0x0001_0000_u32, 0xeeee_eeee];
    let initial = [0x2222_1111_u32, 0xeeee_eeee];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0x2222_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_a1a2_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e16_m1_stride6_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const SOURCE_OFFSET_BYTES: i32 = 0;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 16;
    const STORE_RESULT_OFFSET_BYTES: i32 = 32;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 48;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 64;
    const STRIDE_BYTES: i32 = 6;

    let fail_instruction_index = 29;
    let words = vec![
        u_type(0, 10, 0x17),
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),
        vsetvli_type(0xc8, 11, 5),
        vector_strided_load_type(true, 0b101, 14, 21, 1),
        vector_unit_stride_store_type(true, 0b101, 15, 1),
        vector_strided_store_type(true, 0b101, 16, 21, 1),
        i_type(0, 15, 0b010, 17, 0x03),
        i_type(0, 19, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03),
        i_type(0, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03),
        i_type(4, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03),
        i_type(8, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03),
        i_type(12, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        0x0000_0073,
        0x0000_0000,
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source_span = [0x5151_a1a2_u32, 0xb1b2_6161, 0x7171_7171, 0x8181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xb1b2_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_a1a2_u32, 0xb1b2_4444, 0x5555_6666, 0x7777_8888];

    let mut program = riscv64_program(&program_words);
    for word in source_span
        .into_iter()
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e16_m1_stride6_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = 8;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;
    const STRIDE_BYTES: i32 = 6;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),
        i_type(2, 0, 0b000, 11, 0x13),
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),
        vsetvli_type(0xc8, 11, 5),
        vector_unit_stride_load_type(true, 0b101, 12, 8),
        vector_vi_type(0b011000, 8, 0, 0),
        vector_unit_stride_load_type(true, 0b101, 13, 1),
        vector_strided_load_type(false, 0b101, 14, 21, 1),
        vector_unit_stride_store_type(true, 0b101, 15, 1),
        vector_strided_store_type(false, 0b101, 16, 21, 1),
        i_type(0, 15, 0b010, 17, 0x03),
        i_type(0, 19, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03),
        i_type(0, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03),
        i_type(4, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03),
        i_type(8, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03),
        i_type(12, 20, 0b010, 18, 0x03),
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073,
        0x0000_0000,
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mask = [0x0001_0000_u32, 0xeeee_eeee];
    let initial = [0x2222_1111_u32, 0xeeee_eeee];
    let source_span = [0x5151_a1a2_u32, 0xb1b2_6161, 0x7171_7171, 0x8181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0x2222_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_a1a2_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e32_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 128;
    const STRIDE_BYTES: i32 = 12;
    const SOURCE_BYTES: i32 = 16;

    let fail_instruction_index = 21;
    let words = vec![
        u_type(0, 10, 0x17),                               // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),    // addi x10, x10, data
        i_type(SOURCE_BYTES, 10, 0b000, 13, 0x13),         // addi x13, x10, dest
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 12, 0x13),          // addi x12, x0, stride
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_strided_load_type(true, 0b110, 10, 12, 1),  // vlse32.v v1, (x10), x12
        vector_strided_store_type(true, 0b110, 13, 12, 1), // vsse32.v v1, (x13), x12
        i_type(0, 13, 0b010, 14, 0x03),                    // lw x14, dest[0]
        i_type(0, 10, 0b010, 15, 0x03),                    // lw x15, source[0]
        b_type((fail_instruction_index - 10) * 4, 15, 14, 0b001),
        i_type(12, 13, 0b010, 14, 0x03), // lw x14, dest[12]
        i_type(12, 10, 0b010, 15, 0x03), // lw x15, source[12]
        b_type((fail_instruction_index - 13) * 4, 15, 14, 0b001),
        i_type(4, 13, 0b010, 14, 0x03), // lw x14, dest gap
        i_type(4, 10, 0b010, 15, 0x03), // lw x15, gap sentinel
        b_type((fail_instruction_index - 16) * 4, 15, 14, 0b001),
        i_type(8, 13, 0b010, 14, 0x03), // lw x14, dest gap
        i_type(8, 10, 0b010, 15, 0x03), // lw x15, gap sentinel
        b_type((fail_instruction_index - 19) * 4, 15, 14, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mut program = riscv64_program(&program_words);
    for word in [
        0xa1a2_a3a4_u32,
        0x5151_5151,
        0x6161_6161,
        0xb1b2_b3b4,
        0x1111_1111,
        0x5151_5151,
        0x6161_6161,
        0x2222_2222,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e32_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const VECTOR_BYTES: i32 = 16;
    const COMPACT_BYTES: i32 = 8;
    const STRIDE_BYTES: i32 = 12;

    let fail_instruction_index = 37;
    let words = vec![
        u_type(0, 10, 0x17),                                           // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),                // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                                // addi x12, x10, mask data
        i_type(COMPACT_BYTES, 10, 0b000, 13, 0x13), // addi x13, x10, initial vector
        i_type(VECTOR_BYTES, 10, 0b000, 14, 0x13),  // addi x14, x10, source vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 15, 0x13), // addi x15, x10, load result
        i_type(VECTOR_BYTES * 3, 10, 0b000, 16, 0x13), // addi x16, x10, store result
        i_type(VECTOR_BYTES * 2 + COMPACT_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected load
        i_type(VECTOR_BYTES * 4, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                 // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),      // addi x21, x0, stride
        vsetvli_type(0xd0, 11, 5),                     // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8), // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),             // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 1), // vle32.v v1, (x13)
        vector_strided_load_type(false, 0b110, 14, 21, 1), // vlse32.v v1, (x14), x21, v0.t
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_strided_store_type(false, 0b110, 16, 21, 1), // vsse32.v v1, (x16), x21, v0.t
        i_type(0, 15, 0b010, 17, 0x03),                // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03),                // lw x18, expected lane 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected lane 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 35) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mut program = riscv64_program(&program_words);
    for word in [
        0x0000_0000_u32,
        0x0000_0001,
        0x1111_1111,
        0x2222_2222,
        0xa1a2_a3a4,
        0x5151_5151,
        0x6161_6161,
        0xb1b2_b3b4,
        0x0000_0000,
        0x0000_0000,
        0xa1a2_a3a4,
        0x2222_2222,
        0x7777_7777,
        0x5151_5151,
        0x6161_6161,
        0x8888_8888,
        0xa1a2_a3a4,
        0x5151_5151,
        0x6161_6161,
        0x8888_8888,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e32_m1_stride6_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const VECTOR_BYTES: i32 = 16;
    const STRIDE_BYTES: i32 = 6;

    let fail_instruction_index = 38;
    let words = vec![
        u_type(0, 10, 0x17),                               // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),    // addi x10, x10, data
        i_type(0, 10, 0b000, 14, 0x13),                    // addi x14, x10, source
        i_type(VECTOR_BYTES, 10, 0b000, 15, 0x13),         // addi x15, x10, load result
        i_type(VECTOR_BYTES * 2, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(VECTOR_BYTES * 3, 10, 0b000, 19, 0x13),     // addi x19, x10, expected load
        i_type(VECTOR_BYTES * 4, 10, 0b000, 20, 0x13),     // addi x20, x10, expected store
        i_type(3, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),          // addi x21, x0, stride
        i_type(4, 0, 0b000, 22, 0x13),                     // addi x22, x0, compact vl
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_strided_load_type(true, 0b110, 14, 21, 1),  // vlse32.v v1, (x14), x21
        vsetvli_type(0xd0, 22, 5),                         // vsetvli x5, x22, e32, m1, ta, ma
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_strided_store_type(true, 0b110, 16, 21, 1), // vsse32.v v1, (x16), x21
        i_type(0, 15, 0b010, 17, 0x03),                    // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03),                    // lw x18, expected lane 0
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected lane 1
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b010, 17, 0x03), // lw x17, load result lane 2
        i_type(8, 19, 0b010, 18, 0x03), // lw x18, expected lane 2
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 33) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 36) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mut program = riscv64_program(&program_words);
    for word in [
        0xa1a2_a3a4_u32,
        0x5151_5151,
        0x6161_6161,
        0xc1c2_c3c4,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x1111_1111,
        0x2222_2222,
        0x3333_3333,
        0x4444_4444,
        0xa1a2_a3a4,
        0x6161_5151,
        0xc1c2_c3c4,
        0x0000_0000,
        0xa1a2_a3a4,
        0x5151_2222,
        0x3333_6161,
        0xc1c2_c3c4,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e32_m1_stride6_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const VECTOR_BYTES: i32 = 16;
    const STRIDE_BYTES: i32 = 6;

    let fail_instruction_index = 44;
    let words = vec![
        u_type(0, 10, 0x17),                                // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),     // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                     // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),          // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),      // addi x14, x10, source
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),      // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),      // addi x19, x10, expected load
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),      // addi x20, x10, expected store
        i_type(3, 0, 0b000, 11, 0x13),                      // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),           // addi x21, x0, stride
        i_type(4, 0, 0b000, 22, 0x13),                      // addi x22, x0, compact vl
        vsetvli_type(0xd0, 22, 5),                          // vsetvli x5, x22, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8),   // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                  // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 1),   // vle32.v v1, (x13)
        vsetvli_type(0xd0, 11, 5),                          // vsetvli x5, x11, e32, m1, ta, ma
        vector_strided_load_type(false, 0b110, 14, 21, 1),  // vlse32.v v1, (x14), x21, v0.t
        vsetvli_type(0xd0, 22, 5),                          // vsetvli x5, x22, e32, m1, ta, ma
        vector_unit_stride_store_type(true, 0b110, 15, 1),  // vse32.v v1, (x15)
        vsetvli_type(0xd0, 11, 5),                          // vsetvli x5, x11, e32, m1, ta, ma
        vector_strided_store_type(false, 0b110, 16, 21, 1), // vsse32.v v1, (x16), x21, v0.t
        i_type(0, 15, 0b010, 17, 0x03),                     // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03),                     // lw x18, expected lane 0
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected lane 1
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b010, 17, 0x03), // lw x17, load result lane 2
        i_type(8, 19, 0b010, 18, 0x03), // lw x18, expected lane 2
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 33) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 36) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 39) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 42) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mut program = riscv64_program(&program_words);
    for word in [
        0x0000_0000_u32,
        0x0000_0001,
        0x0000_0000,
        0x0000_0001,
        0x1111_1111,
        0x2222_2222,
        0x3333_3333,
        0x0000_0000,
        0xa1a2_a3a4,
        0x5151_5151,
        0x6161_6161,
        0xc1c2_c3c4,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x0000_0000,
        0x7777_7777,
        0x8888_8888,
        0x9999_9999,
        0xaaaa_aaaa,
        0xa1a2_a3a4,
        0x2222_2222,
        0xc1c2_c3c4,
        0x0000_0000,
        0xa1a2_a3a4,
        0x8888_8888,
        0x9999_9999,
        0xc1c2_c3c4,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn strided_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const VECTOR_BYTES: i32 = 16;
    const STRIDE_BYTES: i32 = 8;

    let fail_instruction_index = 26;
    let words = vec![
        u_type(0, 10, 0x17),                               // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),    // addi x10, x10, data
        i_type(0, 10, 0b000, 14, 0x13),                    // addi x14, x10, source
        i_type(VECTOR_BYTES, 10, 0b000, 15, 0x13),         // addi x15, x10, load result
        i_type(VECTOR_BYTES * 2, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(VECTOR_BYTES * 3, 10, 0b000, 19, 0x13),     // addi x19, x10, expected load
        i_type(VECTOR_BYTES * 4, 10, 0b000, 20, 0x13),     // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),          // addi x21, x0, stride
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_strided_load_type(true, 0b111, 14, 21, 1),  // vlse64.v v1, (x14), x21
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_strided_store_type(true, 0b111, 16, 21, 1), // vsse64.v v1, (x16), x21
        i_type(0, 15, 0b011, 17, 0x03),                    // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03),                    // ld x18, expected lane 0
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected lane 1
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source = [0xa1a2_a3a4_a5a6_a7a8_u64, 0xb1b2_b3b4_b5b6_b7b8];
    let mut program = riscv64_program(&program_words);
    for block in [
        source,
        [0, 0],
        [0x1111_2222_3333_4444, 0x5555_6666_7777_8888],
        source,
        source,
    ] {
        for lane in block {
            program.extend_from_slice(&lane.to_le_bytes());
        }
    }
    program
}

fn sparse_strided_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const SOURCE_OFFSET_BYTES: i32 = 32;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;
    const STRIDE_BYTES: i32 = 24;

    let fail_instruction_index = 32;
    let words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),    // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),                 // addi x21, x0, stride
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_strided_load_type(true, 0b111, 14, 21, 1), // vlse64.v v1, (x14), x21
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_strided_store_type(true, 0b111, 16, 21, 1), // vsse64.v v1, (x16), x21
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected lane 0
        b_type((fail_instruction_index - 15) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected lane 1
        b_type((fail_instruction_index - 18) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(16, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(24, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(24, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x1111_2222_3333_4444,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0x5555_6666_7777_8888,
    ];
    let expected_load = [source_span[0], source_span[3]];
    let mut expected_store = initial_store;
    expected_store[0] = expected_load[0];
    expected_store[3] = expected_load[1];

    let mut program = riscv64_program(&program_words);
    for word in [0xeeee_eeee_eeee_eeee_u64; 4]
        .into_iter()
        .chain(source_span)
        .chain([0_u64, 0])
        .chain([0xeeee_eeee_eeee_eeee_u64; 2])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_strided_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const VECTOR_BYTES: i32 = 16;
    const STRIDE_BYTES: i32 = 8;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),     // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                     // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),          // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),      // addi x14, x10, source
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),      // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),      // addi x19, x10, expected load
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),      // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                      // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),           // addi x21, x0, stride
        vsetvli_type(0xd8, 11, 5),                          // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 8),   // vle64.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                  // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 13, 1),   // vle64.v v1, (x13)
        vector_strided_load_type(false, 0b111, 14, 21, 1),  // vlse64.v v1, (x14), x21, v0.t
        vector_unit_stride_store_type(true, 0b111, 15, 1),  // vse64.v v1, (x15)
        vector_strided_store_type(false, 0b111, 16, 21, 1), // vsse64.v v1, (x16), x21, v0.t
        i_type(0, 15, 0b011, 17, 0x03),                     // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03),                     // ld x18, expected lane 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected lane 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let source = [0xa1a2_a3a4_a5a6_a7a8_u64, 0xb1b2_b3b4_b5b6_b7b8];
    let initial = [0x1111_2222_3333_4444_u64, 0x5555_6666_7777_8888];
    let store = [0x9999_aaaa_bbbb_cccc_u64, 0xdddd_eeee_ffff_0001];
    let mut program = riscv64_program(&program_words);
    for block in [
        [0, 1],
        initial,
        source,
        [0, 0],
        store,
        [source[0], initial[1]],
        [source[0], store[1]],
    ] {
        for lane in block {
            program.extend_from_slice(&lane.to_le_bytes());
        }
    }
    program
}

fn masked_sparse_strided_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_OFFSET_BYTES: i32 = 16;
    const SOURCE_OFFSET_BYTES: i32 = 32;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;
    const STRIDE_BYTES: i32 = 24;

    let fail_instruction_index = 37;
    let words = vec![
        u_type(0, 10, 0x17),                                      // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),           // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),           // addi x12, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 13, 0x13),        // addi x13, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),         // addi x14, x10, source
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),    // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),   // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                            // addi x11, x0, vl
        i_type(STRIDE_BYTES, 0, 0b000, 21, 0x13),                 // addi x21, x0, stride
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 8), // vle64.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 13, 1), // vle64.v v1, (x13)
        vector_strided_load_type(false, 0b111, 14, 21, 1), // vlse64.v v1, (x14), x21, v0.t
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_strided_store_type(false, 0b111, 16, 21, 1), // vsse64.v v1, (x16), x21, v0.t
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected lane 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected lane 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(16, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        i_type(24, 16, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(24, 20, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 35) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let initial = [0x1111_2222_3333_4444_u64, 0x5555_6666_7777_8888];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x9999_aaaa_bbbb_cccc,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0xdddd_eeee_ffff_0001,
    ];
    let expected_load = [source_span[0], initial[1]];
    let mut expected_store = initial_store;
    expected_store[0] = expected_load[0];

    let mut program = riscv64_program(&program_words);
    for word in [0_u64, 1]
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0_u64, 0])
        .chain([0xeeee_eeee_eeee_eeee_u64; 2])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn indexed_e8_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 30;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2), // vle8.v v2, (x12)
        vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1), // vluxei8.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b000, 15, 1), // vse8.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1), // vsuxei8.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 16) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 19) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_22a1_u32, 0x3333_4444, 0x5555_6666, 0xb177_8888];

    let mut program = riscv64_program(&program_words);
    program.extend(index_offsets);
    for word in source_span
        .into_iter()
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_indexed_e8_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 35;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2),  // vle8.v v2, (x12)
        vector_unit_stride_load_type(true, 0b000, 13, 8),  // vle8.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b000, 14, 1),  // vle8.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b000, 15, 2, 1), // vluxei8.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b000, 16, 1), // vse8.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b000, 19, 2, 1), // vsuxei8.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 33) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 15, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mask = [0xeeee_0100_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    program.extend(index_offsets);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2), // vle16.v v2, (x12)
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1), // vluxei16.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b000, 15, 1), // vse8.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1), // vsuxei16.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 15, 0, 0, 0, 0, 0, 0];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_22a1_u32, 0x3333_4444, 0x5555_6666, 0xb177_8888];

    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e8_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 36;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2),  // vle16.v v2, (x12)
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 13, 8),  // vle8.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b000, 14, 1),  // vle8.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b101, 15, 2, 1), // vluxei16.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b000, 16, 1),          // vse8.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b101, 19, 2, 1), // vsuxei16.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 15, 0, 0, 0, 0, 0, 0];
    let mask = [0xeeee_0100_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2), // vle32.v v2, (x12)
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1), // vluxei32.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b000, 15, 1), // vse8.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1), // vsuxei32.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u32, 15, 0, 0];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_22a1_u32, 0x3333_4444, 0x5555_6666, 0xb177_8888];

    let mut program = riscv64_program(&program_words);
    for word in index_offsets
        .into_iter()
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e8_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 36;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2),  // vle32.v v2, (x12)
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 13, 8),  // vle8.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b000, 14, 1),  // vle8.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b110, 15, 2, 1), // vluxei32.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b000, 16, 1),          // vse8.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b110, 19, 2, 1), // vsuxei32.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u32, 15, 0, 0];
    let mask = [0xeeee_0100_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in index_offsets
        .into_iter()
        .chain(mask)
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2), // vle64.v v2, (x12)
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1), // vluxei64.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b000, 15, 1), // vse8.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1), // vsuxei64.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u64, 15];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xeeee_b1a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_22a1_u32, 0x3333_4444, 0x5555_6666, 0xb177_8888];

    let mut program = riscv64_program(&program_words);
    for doubleword in index_offsets {
        program.extend_from_slice(&doubleword.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e8_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 36;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2),  // vle64.v v2, (x12)
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 13, 8),  // vle8.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b000, 14, 1),  // vle8.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b111, 15, 2, 1), // vluxei64.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b000, 16, 1),          // vse8.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b111, 19, 2, 1), // vsuxei64.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u64, 15];
    let mask = [0xeeee_0100_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0xeeee_2211_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_51a1_u32, 0x6161_6161, 0x7171_7171, 0xb181_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0xeeee_22a1_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_aaa1_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for doubleword in index_offsets {
        program.extend_from_slice(&doubleword.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0xeeee_eeee_u32; 4])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn indexed_e16_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 30;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2), // vle16.v v2, (x12)
        vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1), // vluxei16.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b101, 15, 1), // vse16.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1), // vsuxei16.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 16) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 19) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 14, 0, 0, 0, 0, 0, 0];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xb1b2_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_a1a2_u32, 0x3333_4444, 0x5555_6666, 0xb1b2_8888];

    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_indexed_e16_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 35;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2),  // vle16.v v2, (x12)
        vector_unit_stride_load_type(true, 0b101, 13, 8),  // vle16.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b101, 14, 1),  // vle16.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b101, 15, 2, 1), // vluxei16.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b101, 16, 1),          // vse16.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b101, 19, 2, 1), // vsuxei16.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 33) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 14, 0, 0, 0, 0, 0, 0];
    let mask = [0x0001_0000_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0x2222_1111_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0x2222_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_a1a2_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2), // vle8.v v2, (x12)
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1), // vluxei8.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b101, 15, 1), // vse16.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1), // vsuxei8.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xb1b2_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_a1a2_u32, 0x3333_4444, 0x5555_6666, 0xb1b2_8888];

    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in source_span
        .into_iter()
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e16_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 36;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2),  // vle8.v v2, (x12)
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 13, 8),  // vle16.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b101, 14, 1),  // vle16.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b000, 15, 2, 1), // vluxei8.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b101, 16, 1), // vse16.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b000, 19, 2, 1), // vsuxei8.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mask = [0x0001_0000_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0x2222_1111_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0x2222_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_a1a2_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2), // vle32.v v2, (x12)
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1), // vluxei32.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b101, 15, 1), // vse16.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1), // vsuxei32.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u32, 14, 0, 0];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xb1b2_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_a1a2_u32, 0x3333_4444, 0x5555_6666, 0xb1b2_8888];

    let mut program = riscv64_program(&program_words);
    for word in index_offsets
        .into_iter()
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e16_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 36;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2),  // vle32.v v2, (x12)
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 13, 8),  // vle16.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b101, 14, 1),  // vle16.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b110, 15, 2, 1), // vluxei32.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b101, 16, 1),          // vse16.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b110, 19, 2, 1), // vsuxei32.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u32, 14, 0, 0];
    let mask = [0x0001_0000_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0x2222_1111_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0x2222_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_a1a2_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for word in index_offsets
        .into_iter()
        .chain(mask)
        .chain(initial)
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 31;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2), // vle64.v v2, (x12)
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1), // vluxei64.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b101, 15, 1), // vse16.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1), // vsuxei64.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result word 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load word 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u64, 14];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x1111_2222_u32, 0x3333_4444, 0x5555_6666, 0x7777_8888];
    let expected_load = [0xb1b2_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x1111_a1a2_u32, 0x3333_4444, 0x5555_6666, 0xb1b2_8888];

    let mut program = riscv64_program(&program_words);
    for index_offset in index_offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e16_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 36;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2),  // vle64.v v2, (x12)
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 13, 8),  // vle16.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b101, 14, 1),  // vle16.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b111, 15, 2, 1), // vluxei64.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b101, 16, 1),          // vse16.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b111, 19, 2, 1), // vsuxei64.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result word 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load word 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result word 0
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected store word 0
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result word 1
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected store word 1
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result word 2
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected store word 2
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result word 3
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected store word 3
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u64, 14];
    let mask = [0x0001_0000_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0x2222_1111_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0x5151_a1a2_u32, 0x6161_6161, 0x7171_7171, 0xb1b2_8181];
    let initial_store = [0x9999_aaaa_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];
    let expected_load = [0x2222_a1a2_u32, 0xeeee_eeee, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0x9999_a1a2_u32, 0xbbbb_cccc, 0xdddd_eeee, 0xffff_0001];

    let mut program = riscv64_program(&program_words);
    for index_offset in index_offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0_u32, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2), // vle8.v v2, (x12)
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1), // vluxei8.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1), // vsuxei8.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_span = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252];
    let initial_store = [0x1111_1111_u32, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let expected_load = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x6262_6262, 0x2222_2222];

    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in source_span
        .into_iter()
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2),  // vle8.v v2, (x12)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b000, 15, 2, 1), // vluxei8.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b000, 19, 2, 1), // vsuxei8.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mask = [0_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    let initial = [0x1111_1111_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252];
    let initial_store = [0x9999_9999_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    let expected_load = [0xa1a2_a3a4_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xa1a2_a3a4_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];

    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in mask
        .into_iter()
        .chain(initial)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2), // vle16.v v2, (x12)
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1), // vluxei16.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1), // vsuxei16.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 4, 0, 0, 0, 0, 0, 0];
    let source_span = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252];
    let initial_store = [0x1111_1111_u32, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let expected_load = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x6262_6262, 0x2222_2222];

    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2),  // vle16.v v2, (x12)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b101, 15, 2, 1), // vluxei16.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1),          // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b101, 19, 2, 1), // vsuxei16.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 4, 0, 0, 0, 0, 0, 0];
    let mask = [0_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    let initial_vector = [0x1111_1111_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252];
    let initial_store = [0x9999_9999_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    let expected_load = [0xa1a2_a3a4_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xa1a2_a3a4_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];

    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2), // vle64.v v2, (x12)
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1), // vluxei64.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1), // vsuxei64.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u64, 4];
    let source_span = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252];
    let initial_store = [0x1111_1111_u32, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let expected_load = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x6262_6262, 0x2222_2222];

    let mut program = riscv64_program(&program_words);
    for index_offset in index_offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2),  // vle64.v v2, (x12)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b111, 15, 2, 1), // vluxei64.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1),          // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b111, 19, 2, 1), // vsuxei64.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u64, 4];
    let mask = [0_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    let initial_vector = [0x1111_1111_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0xa1a2_a3a4_u32, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252];
    let initial_store = [0x9999_9999_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    let expected_load = [0xa1a2_a3a4_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let expected_store = [0xa1a2_a3a4_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];

    let mut program = riscv64_program(&program_words);
    for index_offset in index_offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    indexed_e32_m1_vector_memory_program_with_offsets(
        [0, 4],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252],
    )
}

fn sparse_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    indexed_e32_m1_vector_memory_program_with_offsets(
        [0, 12],
        [0xa1a2_a3a4, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4],
    )
}

fn leading_gap_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    indexed_e32_m1_vector_memory_program_with_offsets(
        [4, 12],
        [0x5151_5151, 0xa1a2_a3a4, 0x5252_5252, 0xb1b2_b3b4],
    )
}

fn reversed_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    indexed_e32_m1_vector_memory_program_with_offsets(
        [12, 0],
        [0xb1b2_b3b4, 0x5151_5151, 0x5252_5252, 0xa1a2_a3a4],
    )
}

fn indexed_e32_m1_vector_memory_program_with_offsets(
    offsets: [u32; 2],
    source_span: [u32; 4],
) -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 33;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2), // vle32.v v2, (x12)
        vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1), // vluxei32.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1), // vsuxei32.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 16) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 19) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let initial_store = [0x1111_1111, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let mut expected_load = [0xeeee_eeee; 4];
    let mut expected_store = initial_store;
    for (lane, offset) in offsets.into_iter().enumerate() {
        assert_eq!(offset % 4, 0);
        let memory_word_index = usize::try_from(offset / 4).unwrap();
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let mut program = riscv64_program(&program_words);
    for word in offsets
        .into_iter()
        .chain([0xeeee_eeee, 0xeeee_eeee])
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn indexed_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 27;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2), // vle64.v v2, (x12)
        vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1), // vluxei64.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1), // vsuxei64.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 16) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 19) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mut program = riscv64_program(&program_words);
    for word in [
        0_u64,
        8,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2), // vle64.v v2, (x12)
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1), // vluxei64.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1), // vsuxei64.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 12];
    let source_span = [0xa1a2_a3a4_u32, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4];
    let initial_store = [0x1111_1111_u32, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let mut expected_load = [0xeeee_eeee; 4];
    let mut expected_store = initial_store;
    for (lane, offset) in offsets.into_iter().enumerate() {
        assert_eq!(offset % 4, 0);
        let memory_word_index = usize::try_from(offset / 4).unwrap();
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let mut program = riscv64_program(&program_words);
    for index_offset in offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2), // vle16.v v2, (x12)
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1), // vluxei16.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1), // vsuxei16.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 12, 0, 0, 0, 0, 0, 0];
    let source_span = [0xa1a2_a3a4_u32, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4];
    let initial_store = [0x1111_1111_u32, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let mut expected_load = [0xeeee_eeee; 4];
    let mut expected_store = initial_store;
    for (lane, offset) in [index_offsets[0], index_offsets[1]].into_iter().enumerate() {
        assert_eq!(offset % 4, 0);
        let memory_word_index = usize::from(offset / 4);
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let mut program = riscv64_program(&program_words);
    for index_offset in index_offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in source_span
        .into_iter()
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2), // vle8.v v2, (x12)
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1), // vluxei8.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b110, 15, 1), // vse32.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1), // vsuxei8.v v1, (x16), v2
        i_type(0, 15, 0b010, 17, 0x03), // lw x17, load result lane 0
        i_type(0, 19, 0b010, 18, 0x03), // lw x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(4, 15, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 19, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b010, 17, 0x03), // lw x17, store result lane 0
        i_type(0, 20, 0b010, 18, 0x03), // lw x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 20, 0b010, 18, 0x03), // lw x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(12, 16, 0b010, 17, 0x03), // lw x17, store result lane 1
        i_type(12, 20, 0b010, 18, 0x03), // lw x18, expected store lane 1
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let source_span = [0xa1a2_a3a4_u32, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4];
    let initial_store = [0x1111_1111_u32, 0x6161_6161, 0x6262_6262, 0x2222_2222];
    let mut expected_load = [0xeeee_eeee; 4];
    let mut expected_store = initial_store;
    for (lane, offset) in [index_offsets[0], index_offsets[1]].into_iter().enumerate() {
        assert_eq!(offset % 4, 0);
        let memory_word_index = usize::from(offset / 4);
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in source_span
        .into_iter()
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 28;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2), // vle8.v v2, (x12)
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1), // vluxei8.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1), // vsuxei8.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in [
        0xa1a2_a3a4_a5a6_a7a8_u64,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 28;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2), // vle16.v v2, (x12)
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1), // vluxei16.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1), // vsuxei16.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 8, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in [
        0xa1a2_a3a4_a5a6_a7a8_u64,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 16;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 32;
    const STORE_RESULT_OFFSET_BYTES: i32 = 48;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 64;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 80;

    let fail_instruction_index = 28;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2), // vle32.v v2, (x12)
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1), // vluxei32.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1), // vsuxei32.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u32, 8, 0, 0];
    let mut program = riscv64_program(&program_words);
    for word in index_offsets {
        program.extend_from_slice(&word.to_le_bytes());
    }
    for word in [
        0xa1a2_a3a4_a5a6_a7a8_u64,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_indexed_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 32;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 33;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2), // vle64.v v2, (x12)
        vector_indexed_unordered_load_type(true, 0b111, 14, 2, 1), // vluxei64.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b111, 16, 2, 1), // vsuxei64.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 16) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 19) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(16, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(24, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(24, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x1111_2222_3333_4444,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0x5555_6666_7777_8888,
    ];
    let mut expected_load = [0xeeee_eeee_eeee_eeee; 2];
    let mut expected_store = initial_store;
    for (lane, offset) in offsets.into_iter().enumerate() {
        assert_eq!(offset % 8, 0);
        let memory_word_index = usize::try_from(offset / 8).unwrap();
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let mut program = riscv64_program(&program_words);
    for word in offsets
        .into_iter()
        .chain([0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee])
        .chain(source_span)
        .chain([0, 0])
        .chain([0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 32;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5), // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2), // vle8.v v2, (x12)
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b000, 14, 2, 1), // vluxei8.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b000, 16, 2, 1), // vsuxei8.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(16, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(24, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(24, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x1111_2222_3333_4444,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0x5555_6666_7777_8888,
    ];
    let mut expected_load = [0xeeee_eeee_eeee_eeee; 2];
    let mut expected_store = initial_store;
    for (lane, offset) in offsets.into_iter().enumerate() {
        assert_eq!(offset % 8, 0);
        let memory_word_index = usize::try_from(offset / 8).unwrap();
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let index_offsets = [0_u8, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in [0xeeee_eeee_eeee_eeee_u64, 0xeeee_eeee_eeee_eeee]
        .into_iter()
        .chain(source_span)
        .chain([0, 0])
        .chain([0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 32;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2), // vle16.v v2, (x12)
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b101, 14, 2, 1), // vluxei16.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b101, 16, 2, 1), // vsuxei16.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(16, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(24, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(24, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x1111_2222_3333_4444,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0x5555_6666_7777_8888,
    ];
    let mut expected_load = [0xeeee_eeee_eeee_eeee; 2];
    let mut expected_store = initial_store;
    for (lane, offset) in offsets.into_iter().enumerate() {
        assert_eq!(offset % 8, 0);
        let memory_word_index = usize::try_from(offset / 8).unwrap();
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let index_offsets = [0_u16, 24, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in [0xeeee_eeee_eeee_eeee_u64, 0xeeee_eeee_eeee_eeee]
        .into_iter()
        .chain(source_span)
        .chain([0, 0])
        .chain([0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const SOURCE_OFFSET_BYTES: i32 = 32;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 34;
    let words = vec![
        u_type(0, 10, 0x17),                                        // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),             // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),            // addi x12, x10, index offsets
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 14, 0x13),           // addi x14, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 15, 0x13),      // addi x15, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13),     // addi x16, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 19, 0x13),    // addi x19, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 20, 0x13),   // addi x20, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                              // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2), // vle32.v v2, (x12)
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_indexed_unordered_load_type(true, 0b110, 14, 2, 1), // vluxei32.v v1, (x14), v2
        vector_unit_stride_store_type(true, 0b111, 15, 1), // vse64.v v1, (x15)
        vector_indexed_unordered_store_type(true, 0b110, 16, 2, 1), // vsuxei32.v v1, (x16), v2
        i_type(0, 15, 0b011, 17, 0x03), // ld x17, load result lane 0
        i_type(0, 19, 0b011, 18, 0x03), // ld x18, expected load lane 0
        b_type((fail_instruction_index - 17) * 4, 18, 17, 0b001),
        i_type(8, 15, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 19, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 20) * 4, 18, 17, 0b001),
        i_type(0, 16, 0b011, 17, 0x03), // ld x17, store result lane 0
        i_type(0, 20, 0b011, 18, 0x03), // ld x18, expected store lane 0
        b_type((fail_instruction_index - 23) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 26) * 4, 18, 17, 0b001),
        i_type(16, 16, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 20, 0b011, 18, 0x03), // ld x18, expected store gap
        b_type((fail_instruction_index - 29) * 4, 18, 17, 0b001),
        i_type(24, 16, 0b011, 17, 0x03), // ld x17, store result lane 1
        i_type(24, 20, 0b011, 18, 0x03), // ld x18, expected store lane 1
        b_type((fail_instruction_index - 32) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x1111_2222_3333_4444,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0x5555_6666_7777_8888,
    ];
    let mut expected_load = [0xeeee_eeee_eeee_eeee; 2];
    let mut expected_store = initial_store;
    for (lane, offset) in offsets.into_iter().enumerate() {
        assert_eq!(offset % 8, 0);
        let memory_word_index = usize::try_from(offset / 8).unwrap();
        expected_load[lane] = source_span[memory_word_index];
        expected_store[memory_word_index] = expected_load[lane];
    }

    let index_offsets = [0_u32, 24, 0, 0];
    let mut program = riscv64_program(&program_words);
    for word in index_offsets {
        program.extend_from_slice(&word.to_le_bytes());
    }
    for word in [0xeeee_eeee_eeee_eeee_u64, 0xeeee_eeee_eeee_eeee]
        .into_iter()
        .chain(source_span)
        .chain([0, 0])
        .chain([0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_indexed_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 32;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2),  // vle64.v v2, (x12)
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b111, 15, 2, 1), // vluxei64.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1),          // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b111, 19, 2, 1), // vsuxei64.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let mut program = riscv64_program(&program_words);
    for word in [
        0_u64,
        8,
        0,
        1,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x9999_aaaa_bbbb_cccc,
        0xdddd_eeee_ffff_0001,
        0xa1a2_a3a4_a5a6_a7a8,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xdddd_eeee_ffff_0001,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_mixed_width_indexed_e32_m1_data_e64_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2),  // vle64.v v2, (x12)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b111, 15, 2, 1), // vluxei64.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1),          // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b111, 19, 2, 1), // vsuxei64.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 12];
    let mask = [0_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    let initial_vector = [0x1111_1111_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0xa1a2_a3a4_u32, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4];
    let initial_store = [0x9999_9999_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    assert_eq!(offsets[0] % 4, 0);
    let active_word_index = usize::try_from(offsets[0] / 4).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let mut program = riscv64_program(&program_words);
    for index_offset in offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_mixed_width_indexed_e32_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2),  // vle16.v v2, (x12)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b101, 15, 2, 1), // vluxei16.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1),          // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b101, 19, 2, 1), // vsuxei16.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 12, 0, 0, 0, 0, 0, 0];
    let mask = [0_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    let initial_vector = [0x1111_1111_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0xa1a2_a3a4_u32, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4];
    let initial_store = [0x9999_9999_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    assert_eq!(index_offsets[0] % 4, 0);
    let active_word_index = usize::from(index_offsets[0] / 4);
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let mut program = riscv64_program(&program_words);
    for index_offset in index_offsets {
        program.extend_from_slice(&index_offset.to_le_bytes());
    }
    for word in mask
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_mixed_width_indexed_e32_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2),  // vle8.v v2, (x12)
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b000, 15, 2, 1), // vluxei8.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1), // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b000, 19, 2, 1), // vsuxei8.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mask = [0_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    let initial_vector = [0x1111_1111_u32, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let source_span = [0xa1a2_a3a4_u32, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4];
    let initial_store = [0x9999_9999_u32, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    assert_eq!(index_offsets[0] % 4, 0);
    let active_word_index = usize::from(index_offsets[0] / 4);
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in mask
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 33;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2),  // vle8.v v2, (x12)
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b000, 15, 2, 1), // vluxei8.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1), // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b000, 19, 2, 1), // vsuxei8.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u8, 8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in [
        0_u64,
        1,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x9999_aaaa_bbbb_cccc,
        0xdddd_eeee_ffff_0001,
        0xa1a2_a3a4_a5a6_a7a8,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xdddd_eeee_ffff_0001,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 33;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2),  // vle16.v v2, (x12)
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b101, 15, 2, 1), // vluxei16.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1),          // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b101, 19, 2, 1), // vsuxei16.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u16, 8, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in [
        0_u64,
        1,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x9999_aaaa_bbbb_cccc,
        0xdddd_eeee_ffff_0001,
        0xa1a2_a3a4_a5a6_a7a8,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xdddd_eeee_ffff_0001,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 33;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2),  // vle32.v v2, (x12)
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b110, 15, 2, 1), // vluxei32.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1),          // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b110, 19, 2, 1), // vsuxei32.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let index_offsets = [0_u32, 8, 0, 0];
    let mut program = riscv64_program(&program_words);
    for word in index_offsets {
        program.extend_from_slice(&word.to_le_bytes());
    }
    for word in [
        0_u64,
        1,
        0x1111_2222_3333_4444,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xb1b2_b3b4_b5b6_b7b8,
        0,
        0,
        0x9999_aaaa_bbbb_cccc,
        0xdddd_eeee_ffff_0001,
        0xa1a2_a3a4_a5a6_a7a8,
        0x5555_6666_7777_8888,
        0xa1a2_a3a4_a5a6_a7a8,
        0xdddd_eeee_ffff_0001,
    ] {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_indexed_e64_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 80;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 38;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 2),  // vle64.v v2, (x12)
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b111, 15, 2, 1), // vluxei64.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1),          // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b111, 19, 2, 1), // vsuxei64.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        i_type(16, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 33) * 4, 18, 17, 0b001),
        i_type(24, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(24, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 36) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let initial_vector = [0x1111_2222_3333_4444, 0x5555_6666_7777_8888];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x9999_aaaa_bbbb_cccc,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0xdddd_eeee_ffff_0001,
    ];
    assert_eq!(offsets[0] % 8, 0);
    let active_word_index = usize::try_from(offsets[0] / 8).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let mut program = riscv64_program(&program_words);
    for word in offsets
        .into_iter()
        .chain([0, 1])
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_mixed_width_indexed_e64_m1_data_e8_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 80;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc0, 11, 5),                         // vsetvli x5, x11, e8, m1, ta, ma
        vector_unit_stride_load_type(true, 0b000, 12, 2),  // vle8.v v2, (x12)
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b000, 15, 2, 1), // vluxei8.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1), // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b000, 19, 2, 1), // vsuxei8.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(16, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(24, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(24, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let initial_vector = [0x1111_2222_3333_4444, 0x5555_6666_7777_8888];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x9999_aaaa_bbbb_cccc,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0xdddd_eeee_ffff_0001,
    ];
    assert_eq!(offsets[0] % 8, 0);
    let active_word_index = usize::try_from(offsets[0] / 8).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let index_offsets = [0_u8, 24, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    program.extend_from_slice(&index_offsets);
    for word in [0_u64, 1]
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_mixed_width_indexed_e64_m1_data_e16_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 80;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5),                         // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 2),  // vle16.v v2, (x12)
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b101, 15, 2, 1), // vluxei16.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1),          // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b101, 19, 2, 1), // vsuxei16.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(16, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(24, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(24, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let initial_vector = [0x1111_2222_3333_4444, 0x5555_6666_7777_8888];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x9999_aaaa_bbbb_cccc,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0xdddd_eeee_ffff_0001,
    ];
    assert_eq!(offsets[0] % 8, 0);
    let active_word_index = usize::try_from(offsets[0] / 8).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let index_offsets = [0_u16, 24, 0, 0, 0, 0, 0, 0];
    let mut program = riscv64_program(&program_words);
    for halfword in index_offsets {
        program.extend_from_slice(&halfword.to_le_bytes());
    }
    for word in [0_u64, 1]
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_sparse_mixed_width_indexed_e64_m1_data_e32_indices_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 80;
    const STORE_RESULT_OFFSET_BYTES: i32 = 96;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 128;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 144;

    let fail_instruction_index = 39;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2),  // vle32.v v2, (x12)
        vsetvli_type(0xd8, 11, 5),                         // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 13, 8),  // vle64.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 14, 1),  // vle64.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b110, 15, 2, 1), // vluxei32.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 1),          // vse64.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b110, 19, 2, 1), // vsuxei32.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b011, 17, 0x03),                              // ld x17, load result lane 0
        i_type(0, 20, 0b011, 18, 0x03),                              // ld x18, expected load lane 0
        b_type((fail_instruction_index - 22) * 4, 18, 17, 0b001),
        i_type(8, 16, 0b011, 17, 0x03), // ld x17, load result lane 1
        i_type(8, 20, 0b011, 18, 0x03), // ld x18, expected load lane 1
        b_type((fail_instruction_index - 25) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b011, 17, 0x03), // ld x17, store result active lane
        i_type(0, 21, 0b011, 18, 0x03), // ld x18, expected active lane
        b_type((fail_instruction_index - 28) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(8, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 31) * 4, 18, 17, 0b001),
        i_type(16, 19, 0b011, 17, 0x03), // ld x17, store result gap
        i_type(16, 21, 0b011, 18, 0x03), // ld x18, expected gap
        b_type((fail_instruction_index - 34) * 4, 18, 17, 0b001),
        i_type(24, 19, 0b011, 17, 0x03), // ld x17, inactive store lane
        i_type(24, 21, 0b011, 18, 0x03), // ld x18, expected inactive lane
        b_type((fail_instruction_index - 37) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let offsets = [0_u64, 24];
    let initial_vector = [0x1111_2222_3333_4444, 0x5555_6666_7777_8888];
    let source_span = [
        0xa1a2_a3a4_a5a6_a7a8,
        0x5151_5151_5151_5151,
        0x5252_5252_5252_5252,
        0xb1b2_b3b4_b5b6_b7b8,
    ];
    let initial_store = [
        0x9999_aaaa_bbbb_cccc,
        0x6161_6161_6161_6161,
        0x6262_6262_6262_6262,
        0xdddd_eeee_ffff_0001,
    ];
    assert_eq!(offsets[0] % 8, 0);
    let active_word_index = usize::try_from(offsets[0] / 8).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let index_offsets = [0_u32, 24, 0, 0];
    let mut program = riscv64_program(&program_words);
    for word in index_offsets {
        program.extend_from_slice(&word.to_le_bytes());
    }
    for word in [0_u64, 1]
        .into_iter()
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [0, 4],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0x5151_5151, 0x5252_5252],
        0,
    )
}

fn masked_sparse_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [0, 12],
        [0xa1a2_a3a4, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4],
        0,
    )
}

fn masked_leading_gap_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [4, 12],
        [0x5151_5151, 0xa1a2_a3a4, 0x5252_5252, 0xb1b2_b3b4],
        0,
    )
}

fn masked_trailing_active_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [4, 12],
        [0x5151_5151, 0xa1a2_a3a4, 0x5252_5252, 0xb1b2_b3b4],
        1,
    )
}

fn masked_reversed_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [12, 0],
        [0x5151_5151, 0x6161_6161, 0x6262_6262, 0xa1a2_a3a4],
        0,
    )
}

fn masked_indexed_e32_m1_vector_memory_program_with_offsets(
    offsets: [u32; 2],
    source_span: [u32; 4],
    active_lane: usize,
) -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const INDEX_OFFSET_BYTES: i32 = 0;
    const MASK_OFFSET_BYTES: i32 = 16;
    const INITIAL_OFFSET_BYTES: i32 = 32;
    const SOURCE_OFFSET_BYTES: i32 = 48;
    const LOAD_RESULT_OFFSET_BYTES: i32 = 64;
    const STORE_RESULT_OFFSET_BYTES: i32 = 80;
    const EXPECTED_LOAD_OFFSET_BYTES: i32 = 96;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = 112;

    let fail_instruction_index = 38;
    let words = vec![
        u_type(0, 10, 0x17),                                         // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),              // addi x10, x10, data
        i_type(INDEX_OFFSET_BYTES, 10, 0b000, 12, 0x13),             // addi x12, x10, index offsets
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 13, 0x13),              // addi x13, x10, mask data
        i_type(INITIAL_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial vector
        i_type(SOURCE_OFFSET_BYTES, 10, 0b000, 15, 0x13),  // addi x15, x10, source span
        i_type(LOAD_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, store result
        i_type(EXPECTED_LOAD_OFFSET_BYTES, 10, 0b000, 20, 0x13), // addi x20, x10, expected load
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                     // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5),                         // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 2),  // vle32.v v2, (x12)
        vector_unit_stride_load_type(true, 0b110, 13, 8),  // vle32.v v8, (x13)
        vector_vi_type(0b011000, 8, 0, 0),                 // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 14, 1),  // vle32.v v1, (x14)
        vector_indexed_unordered_load_type(false, 0b110, 15, 2, 1), // vluxei32.v v1, (x15), v2, v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 1),          // vse32.v v1, (x16)
        vector_indexed_unordered_store_type(false, 0b110, 19, 2, 1), // vsuxei32.v v1, (x19), v2, v0.t
        i_type(0, 16, 0b010, 17, 0x03),                              // lw x17, load result lane 0
        i_type(0, 20, 0b010, 18, 0x03),                              // lw x18, expected load lane 0
        b_type((fail_instruction_index - 21) * 4, 18, 17, 0b001),
        i_type(4, 16, 0b010, 17, 0x03), // lw x17, load result lane 1
        i_type(4, 20, 0b010, 18, 0x03), // lw x18, expected load lane 1
        b_type((fail_instruction_index - 24) * 4, 18, 17, 0b001),
        i_type(0, 19, 0b010, 17, 0x03), // lw x17, store result active lane
        i_type(0, 21, 0b010, 18, 0x03), // lw x18, expected active lane
        b_type((fail_instruction_index - 27) * 4, 18, 17, 0b001),
        i_type(4, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(4, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 30) * 4, 18, 17, 0b001),
        i_type(8, 19, 0b010, 17, 0x03), // lw x17, store result gap
        i_type(8, 21, 0b010, 18, 0x03), // lw x18, expected gap
        b_type((fail_instruction_index - 33) * 4, 18, 17, 0b001),
        i_type(12, 19, 0b010, 17, 0x03), // lw x17, inactive store lane
        i_type(12, 21, 0b010, 18, 0x03), // lw x18, expected inactive lane
        b_type((fail_instruction_index - 36) * 4, 18, 17, 0b001),
        0x0000_0073, // ecall
        0x0000_0000, // fail: invalid instruction
    ];
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);

    let mut program_words = words;
    while program_words.len() * 4 < DATA_OFFSET_BYTES as usize {
        program_words.push(0);
    }

    let initial_vector = [0x1111_1111, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee];
    let initial_store = [0x9999_9999, 0x6161_6161, 0x6262_6262, 0xaaaa_aaaa];
    assert!(active_lane < 2);
    assert_eq!(offsets[active_lane] % 4, 0);
    let active_word_index = usize::try_from(offsets[active_lane] / 4).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[active_lane] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;
    let mut mask_data = [1_u32, 1, 0xeeee_eeee, 0xeeee_eeee];
    mask_data[active_lane] = 0;

    let mut program = riscv64_program(&program_words);
    for word in offsets
        .into_iter()
        .chain([0xeeee_eeee, 0xeeee_eeee])
        .chain(mask_data)
        .chain(initial_vector)
        .chain(source_span)
        .chain([0, 0, 0xeeee_eeee, 0xeeee_eeee])
        .chain(initial_store)
        .chain(expected_load)
        .chain(expected_store)
    {
        program.extend_from_slice(&word.to_le_bytes());
    }
    program
}

fn masked_unit_stride_lmul4_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 512;
    const WORDS_PER_VECTOR: usize = 16;
    const VECTOR_BYTES: i32 = (WORDS_PER_VECTOR * 4) as i32;

    let fail_instruction_index = 114;
    let mut words = vec![
        u_type(0, 10, 0x17),                                 // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),      // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                      // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),           // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),       // addi x14, x10, source vector
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),       // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),       // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),       // addi x19, x10, expected load result
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),       // addi x20, x10, expected store result
        i_type(WORDS_PER_VECTOR as i32, 0, 0b000, 11, 0x13), // addi x11, x0, vl
        vsetvli_type(0xd2, 11, 5),                           // vsetvli x5, x11, e32, m4, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8),    // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                   // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 4),    // vle32.v v4, (x13)
        vector_unit_stride_load_type(false, 0b110, 14, 4),   // vle32.v v4, (x14), v0.t
        vector_unit_stride_store_type(true, 0b110, 15, 4),   // vse32.v v4, (x15)
        vector_unit_stride_store_type(false, 0b110, 16, 4),  // vse32.v v4, (x16), v0.t
    ];

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, store result
        words.push(i_type(offset, 20, 0b010, 18, 0x03)); // lw x18, expected store result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mut program = riscv64_program(&words);
    for vector in masked_lmul4_vector_memory_data_blocks() {
        for word in vector {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_lmul4_vector_memory_data_blocks() -> [[u32; 16]; 7] {
    let mut vectors = [[0; 16]; 7];
    for lane in 0..16 {
        let active = lane % 2 == 0;
        vectors[0][lane] = u32::from(!active);
        vectors[1][lane] = 0x1100_0000 + lane as u32;
        vectors[2][lane] = 0xa100_0000 + lane as u32;
        vectors[4][lane] = 0x5100_0000 + lane as u32;
        vectors[5][lane] = if active {
            vectors[2][lane]
        } else {
            vectors[1][lane]
        };
        vectors[6][lane] = if active {
            vectors[2][lane]
        } else {
            vectors[4][lane]
        };
    }
    vectors
}

fn masked_unit_stride_lmul8_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 1024;
    const WORDS_PER_VECTOR: usize = 32;
    const VECTOR_BYTES: i32 = (WORDS_PER_VECTOR * 4) as i32;

    let fail_instruction_index = 17 + WORDS_PER_VECTOR as i32 * 6 + 1;
    let mut words = vec![
        u_type(0, 10, 0x17),                                 // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),      // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                      // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),           // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),       // addi x14, x10, source vector
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),       // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),       // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),       // addi x19, x10, expected load result
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),       // addi x20, x10, expected store result
        i_type(WORDS_PER_VECTOR as i32, 0, 0b000, 11, 0x13), // addi x11, x0, vl
        vsetvli_type(0xd3, 11, 5),                           // vsetvli x5, x11, e32, m8, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8),    // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0),                   // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 16),   // vle32.v v16, (x13)
        vector_unit_stride_load_type(false, 0b110, 14, 16),  // vle32.v v16, (x14), v0.t
        vector_unit_stride_store_type(true, 0b110, 15, 16),  // vse32.v v16, (x15)
        vector_unit_stride_store_type(false, 0b110, 16, 16), // vse32.v v16, (x16), v0.t
    ];

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, store result
        words.push(i_type(offset, 20, 0b010, 18, 0x03)); // lw x18, expected store result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mut program = riscv64_program(&words);
    for vector in masked_lmul8_vector_memory_data_blocks() {
        for word in vector {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_lmul8_vector_memory_data_blocks() -> [[u32; 32]; 7] {
    let mut vectors = [[0; 32]; 7];
    for lane in 0..32 {
        let active = lane % 2 == 0;
        vectors[0][lane] = u32::from(!active);
        vectors[1][lane] = 0x1100_0000 + lane as u32;
        vectors[2][lane] = 0xa100_0000 + lane as u32;
        vectors[4][lane] = 0x5100_0000 + lane as u32;
        vectors[5][lane] = if active {
            vectors[2][lane]
        } else {
            vectors[1][lane]
        };
        vectors[6][lane] = if active {
            vectors[2][lane]
        } else {
            vectors[4][lane]
        };
    }
    vectors
}

fn masked_unit_stride_lmul2_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 320;
    const WORDS_PER_VECTOR: usize = 8;
    const VECTOR_BYTES: i32 = (WORDS_PER_VECTOR * 4) as i32;

    let fail_instruction_index = 66;
    let mut words = vec![
        u_type(0, 10, 0x17),                                 // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),      // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                      // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),           // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),       // addi x14, x10, source vector
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),       // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),       // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),       // addi x19, x10, expected load result
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),       // addi x20, x10, expected store result
        i_type(WORDS_PER_VECTOR as i32, 0, 0b000, 11, 0x13), // addi x11, x0, vl
        vsetvli_type(0xd1, 11, 5),                           // vsetvli x5, x11, e32, m2, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 4),    // vle32.v v4, (x12)
        vector_vi_type(0b011000, 4, 0, 0),                   // vmseq.vi v0, v4, 0
        vector_unit_stride_load_type(true, 0b110, 13, 2),    // vle32.v v2, (x13)
        vector_unit_stride_load_type(false, 0b110, 14, 2),   // vle32.v v2, (x14), v0.t
        vector_unit_stride_store_type(true, 0b110, 15, 2),   // vse32.v v2, (x15)
        vector_unit_stride_store_type(false, 0b110, 16, 2),  // vse32.v v2, (x16), v0.t
    ];

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 15, 0b010, 17, 0x03)); // lw x17, load result
        words.push(i_type(offset, 19, 0b010, 18, 0x03)); // lw x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    for word_index in 0..WORDS_PER_VECTOR {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 17, 0x03)); // lw x17, store result
        words.push(i_type(offset, 20, 0b010, 18, 0x03)); // lw x18, expected store result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let vectors: [[u32; WORDS_PER_VECTOR]; 7] = [
        [0, 1, 0, 1, 0, 1, 0, 1],
        [
            0x1111_1111,
            0x2222_2222,
            0x3333_3333,
            0x4444_4444,
            0x5555_5555,
            0x6666_6666,
            0x7777_7777,
            0x8888_8888,
        ],
        [
            0xa1a2_a3a4,
            0xb1b2_b3b4,
            0xc1c2_c3c4,
            0xd1d2_d3d4,
            0xe1e2_e3e4,
            0xf1f2_f3f4,
            0x1718_191a,
            0x2728_292a,
        ],
        [0; WORDS_PER_VECTOR],
        [
            0x5151_5151,
            0x5252_5252,
            0x5353_5353,
            0x5454_5454,
            0x5555_5555,
            0x5656_5656,
            0x5757_5757,
            0x5858_5858,
        ],
        [
            0xa1a2_a3a4,
            0x2222_2222,
            0xc1c2_c3c4,
            0x4444_4444,
            0xe1e2_e3e4,
            0x6666_6666,
            0x1718_191a,
            0x8888_8888,
        ],
        [
            0xa1a2_a3a4,
            0x5252_5252,
            0xc1c2_c3c4,
            0x5454_5454,
            0xe1e2_e3e4,
            0x5656_5656,
            0x1718_191a,
            0x5858_5858,
        ],
    ];

    let mut program = riscv64_program(&words);
    for vector in vectors {
        for word in vector {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_e16_lmul2_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 512;
    const LANES_PER_VECTOR: usize = 16;
    const ELEMENT_BYTES: i32 = 2;
    const VECTOR_BYTES: i32 = LANES_PER_VECTOR as i32 * ELEMENT_BYTES;

    let fail_instruction_index = 18 + LANES_PER_VECTOR as i32 * 6;
    let mut words = vec![
        u_type(0, 10, 0x17),                                 // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),      // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                      // addi x12, x10, mask data
        i_type(VECTOR_BYTES, 10, 0b000, 13, 0x13),           // addi x13, x10, initial vector
        i_type(VECTOR_BYTES * 2, 10, 0b000, 14, 0x13),       // addi x14, x10, source vector
        i_type(VECTOR_BYTES * 3, 10, 0b000, 15, 0x13),       // addi x15, x10, load result
        i_type(VECTOR_BYTES * 4, 10, 0b000, 16, 0x13),       // addi x16, x10, store result
        i_type(VECTOR_BYTES * 5, 10, 0b000, 19, 0x13),       // addi x19, x10, expected load result
        i_type(VECTOR_BYTES * 6, 10, 0b000, 20, 0x13),       // addi x20, x10, expected store result
        i_type(LANES_PER_VECTOR as i32, 0, 0b000, 11, 0x13), // addi x11, x0, vl
        vsetvli_type(0xc9, 11, 5),                           // vsetvli x5, x11, e16, m2, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 4),    // vle16.v v4, (x12)
        vector_vi_type(0b011000, 4, 0, 0),                   // vmseq.vi v0, v4, 0
        vector_unit_stride_load_type(true, 0b101, 13, 2),    // vle16.v v2, (x13)
        vector_unit_stride_load_type(false, 0b101, 14, 2),   // vle16.v v2, (x14), v0.t
        vector_unit_stride_store_type(true, 0b101, 15, 2),   // vse16.v v2, (x15)
        vector_unit_stride_store_type(false, 0b101, 16, 2),  // vse16.v v2, (x16), v0.t
    ];

    for lane_index in 0..LANES_PER_VECTOR {
        let offset = (lane_index * ELEMENT_BYTES as usize) as i32;
        words.push(i_type(offset, 15, 0b101, 17, 0x03)); // lhu x17, load result
        words.push(i_type(offset, 19, 0b101, 18, 0x03)); // lhu x18, expected load result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    for lane_index in 0..LANES_PER_VECTOR {
        let offset = (lane_index * ELEMENT_BYTES as usize) as i32;
        words.push(i_type(offset, 16, 0b101, 17, 0x03)); // lhu x17, store result
        words.push(i_type(offset, 20, 0b101, 18, 0x03)); // lhu x18, expected store result
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mask = [0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1, 0, 1];
    let initial = [
        0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888, 0x9999, 0xaaaa, 0xbbbb,
        0xcccc, 0xdddd, 0xeeee, 0xffff, 0x0101,
    ];
    let source = [
        0xa1a2, 0xb1b2, 0xc1c2, 0xd1d2, 0xe1e2, 0xf1f2, 0x1718, 0x2728, 0x3738, 0x4748, 0x5758,
        0x6768, 0x7778, 0x8788, 0x9798, 0xa7a8,
    ];
    let store_initial = [
        0x5151, 0x5252, 0x5353, 0x5454, 0x5555, 0x5656, 0x5757, 0x5858, 0x5959, 0x5a5a, 0x5b5b,
        0x5c5c, 0x5d5d, 0x5e5e, 0x5f5f, 0x6060,
    ];
    let mut expected_load = initial;
    let mut expected_store = store_initial;
    for lane_index in (0..LANES_PER_VECTOR).step_by(2) {
        expected_load[lane_index] = source[lane_index];
        expected_store[lane_index] = source[lane_index];
    }

    let mut program = riscv64_program(&words);
    for vector in [
        mask,
        initial,
        source,
        [0; LANES_PER_VECTOR],
        store_initial,
        expected_load,
        expected_store,
    ] {
        program.extend_from_slice(&u16_halfwords_to_bytes(&vector));
    }
    program
}

fn unit_stride_segment_e32_m1_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xd0,
        2,
        2,
        0b110,
        16,
        4,
        0b010,
        &u32_words_to_bytes(&[0x1111_1111, 0x2222_2222, 0x3333_3333, 0x4444_4444]),
    )
}

fn unit_stride_segment_e32_m1_three_field_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xd0,
        1,
        3,
        0b110,
        12,
        4,
        0b010,
        &u32_words_to_bytes(&[0x1111_1111, 0x2222_2222, 0x3333_3333]),
    )
}

fn unit_stride_segment_e32_m1_four_field_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xd0,
        2,
        4,
        0b110,
        32,
        4,
        0b010,
        &u32_words_to_bytes(&[
            0x1111_1111,
            0x2222_2222,
            0x3333_3333,
            0x4444_4444,
            0x5555_5555,
            0x6666_6666,
            0x7777_7777,
            0x8888_8888,
        ]),
    )
}

fn unit_stride_segment_e8_m1_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xc0,
        8,
        2,
        0b000,
        16,
        4,
        0b010,
        &[
            0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd, 0xee,
            0xff, 0x01,
        ],
    )
}

fn unit_stride_segment_e16_m1_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xc8,
        4,
        2,
        0b101,
        16,
        4,
        0b010,
        &u16_halfwords_to_bytes(&[
            0x1111, 0x2222, 0x3333, 0x4444, 0x5555, 0x6666, 0x7777, 0x8888,
        ]),
    )
}

fn unit_stride_segment_e64_m1_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xd8,
        1,
        2,
        0b111,
        16,
        8,
        0b011,
        &u64_words_to_bytes(&[0x1111_2222_3333_4444, 0x5555_6666_7777_8888]),
    )
}

fn unit_stride_segment_e64_m1_two_line_vector_memory_program() -> Vec<u8> {
    unit_stride_segment_m1_vector_memory_program(
        0xd8,
        2,
        2,
        0b111,
        32,
        8,
        0b011,
        &u64_words_to_bytes(&[
            0x1111_2222_3333_4444,
            0x5555_6666_7777_8888,
            0x9999_aaaa_bbbb_cccc,
            0xdddd_eeee_ffff_0001,
        ]),
    )
}

fn unit_stride_segment_e16_m2_full_register_group_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 512;
    const LANES_PER_FIELD: usize = 16;
    const FIELDS: usize = 2;
    const SEGMENT_HALFWORDS: usize = LANES_PER_FIELD * FIELDS;
    const SEGMENT_BYTES: i32 = (SEGMENT_HALFWORDS * 2) as i32;
    const SOURCE_SEGMENT_OFFSET_BYTES: i32 = 0;
    const STORE_RESULT_OFFSET_BYTES: i32 = SEGMENT_BYTES;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = SEGMENT_BYTES * 2;
    const FAIL_INSTRUCTION_INDEX: i32 = 106;

    let mut words = vec![
        u_type(0, 10, 0x17),                                          // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),               // addi x10, x10, data
        i_type(SOURCE_SEGMENT_OFFSET_BYTES, 10, 0b000, 12, 0x13), // addi x12, x10, source segment
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 13, 0x13),   // addi x13, x10, store result
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, expected result
        i_type(LANES_PER_FIELD as i32, 0, 0b000, 11, 0x13),       // addi x11, x0, vl
        vsetvli_type(0xc9, 11, 5), // vsetvli x5, x11, e16, m2, ta, ma
        vector_unit_stride_segment_load_type(true, 2, 0b101, 12, 2), // vlseg2e16.v v2, (x12)
        vector_unit_stride_segment_store_type(true, 2, 0b101, 13, 2), // vsseg2e16.v v2, (x13)
    ];

    for halfword_index in 0..SEGMENT_HALFWORDS {
        let offset = (halfword_index * 2) as i32;
        words.push(i_type(offset, 13, 0b101, 17, 0x03)); // lhu x17, observed halfword
        words.push(i_type(offset, 19, 0b101, 18, 0x03)); // lhu x18, expected halfword
        let branch_index = words.len() as i32;
        words.push(b_type(
            (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mut source_segment = Vec::with_capacity(SEGMENT_HALFWORDS);
    for lane in 0..LANES_PER_FIELD {
        source_segment.push(0xa100 + lane as u16);
        source_segment.push(0xb200 + lane as u16);
    }

    let mut program = riscv64_program(&words);
    program.extend_from_slice(&u16_halfwords_to_bytes(&source_segment));
    program.extend_from_slice(&u16_halfwords_to_bytes(&[0xeeee; SEGMENT_HALFWORDS]));
    program.extend_from_slice(&u16_halfwords_to_bytes(&source_segment));
    program
}

fn unit_stride_segment_m1_vector_memory_program(
    vtype: u32,
    avl: i32,
    fields: u32,
    vector_width: u32,
    segment_bytes: i32,
    compare_step: i32,
    compare_funct3: u32,
    segment_bytes_payload: &[u8],
) -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 192;
    let block_bytes = ((segment_bytes + 15) / 16) * 16;
    let compare_count = segment_bytes / compare_step;
    let fail_instruction_index = 9 + compare_count * 3 + 1;
    let mut words = vec![
        u_type(0, 10, 0x17),                            // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                 // addi x12, x10, source segment
        i_type(block_bytes, 10, 0b000, 13, 0x13),       // addi x13, x10, store result
        i_type(block_bytes * 2, 10, 0b000, 19, 0x13),   // addi x19, x10, expected result
        i_type(avl, 0, 0b000, 11, 0x13),                // addi x11, x0, vl
        vsetvli_type(vtype, 11, 5),                     // vsetvli x5, x11, selected SEW, m1, ta, ma
        vector_unit_stride_segment_load_type(true, fields, vector_width, 12, 2), // vlseg*e*.v v2, (x12)
        vector_unit_stride_segment_store_type(true, fields, vector_width, 13, 2), // vsseg*e*.v v2, (x13)
    ];

    for compare_index in 0..compare_count {
        let offset = compare_index * compare_step;
        words.push(i_type(offset, 13, compare_funct3, 17, 0x03)); // load observed bytes
        words.push(i_type(offset, 19, compare_funct3, 18, 0x03)); // load expected bytes
        let branch_index = words.len() as i32;
        words.push(b_type(
            (fail_instruction_index - branch_index) * 4,
            18,
            17,
            0b001,
        ));
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, fail_instruction_index + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    assert_eq!(segment_bytes_payload.len(), segment_bytes as usize);
    let mut program = riscv64_program(&words);
    program.extend_from_slice(segment_bytes_payload);
    program.resize(DATA_OFFSET_BYTES as usize + block_bytes as usize, 0);
    program.resize(DATA_OFFSET_BYTES as usize + block_bytes as usize * 2, 0);
    program.extend_from_slice(segment_bytes_payload);
    program
}

fn u16_halfwords_to_bytes(halfwords: &[u16]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(halfwords.len() * 2);
    for halfword in halfwords {
        bytes.extend_from_slice(&halfword.to_le_bytes());
    }
    bytes
}

fn u32_words_to_bytes(words: &[u32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 4);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn u64_words_to_bytes(words: &[u64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words.len() * 8);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes
}

fn masked_unit_stride_segment_e32_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_FIELD0_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const INITIAL_FIELD1_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const SOURCE_SEGMENT_OFFSET_BYTES: i32 = BLOCK_BYTES * 3;
    const LOAD_FIELD0_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const LOAD_FIELD1_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 5;
    const STORE_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 6;
    const EXPECTED_FIELD0_OFFSET_BYTES: i32 = BLOCK_BYTES * 7;
    const EXPECTED_FIELD1_OFFSET_BYTES: i32 = BLOCK_BYTES * 8;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 9;
    const FAIL_INSTRUCTION_INDEX: i32 = 47;

    let mut words = vec![
        u_type(0, 10, 0x17),                                           // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),                // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),                // addi x12, x10, mask data
        i_type(INITIAL_FIELD0_OFFSET_BYTES, 10, 0b000, 13, 0x13), // addi x13, x10, initial field 0
        i_type(INITIAL_FIELD1_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial field 1
        i_type(SOURCE_SEGMENT_OFFSET_BYTES, 10, 0b000, 15, 0x13), // addi x15, x10, source segment
        i_type(LOAD_FIELD0_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load field 0 result
        i_type(LOAD_FIELD1_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, load field 1 result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 20, 0x13),       // addi x20, x10, store result
        i_type(EXPECTED_FIELD0_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected field 0
        i_type(EXPECTED_FIELD1_OFFSET_BYTES, 10, 0b000, 22, 0x13), // addi x22, x10, expected field 1
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 23, 0x13),  // addi x23, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                             // addi x11, x0, vl
        vsetvli_type(0xd0, 11, 5), // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 8), // vle32.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b110, 13, 2), // vle32.v v2, (x13)
        vector_unit_stride_load_type(true, 0b110, 14, 3), // vle32.v v3, (x14)
        vector_unit_stride_segment_load_type(false, 2, 0b110, 15, 2), // vlseg2e32.v v2, (x15), v0.t
        vector_unit_stride_store_type(true, 0b110, 16, 2), // vse32.v v2, (x16)
        vector_unit_stride_store_type(true, 0b110, 19, 3), // vse32.v v3, (x19)
        vector_unit_stride_segment_store_type(false, 2, 0b110, 20, 2), // vsseg2e32.v v2, (x20), v0.t
    ];

    for (base, expected_base, words_to_compare) in [(16, 21, 2), (19, 22, 2), (20, 23, 4)] {
        for word_index in 0..words_to_compare {
            let offset = (word_index * 4) as i32;
            words.push(i_type(offset, base, 0b010, 17, 0x03)); // lw x17, observed word
            words.push(i_type(offset, expected_base, 0b010, 18, 0x03)); // lw x18, expected word
            let branch_index = words.len() as i32;
            words.push(b_type(
                (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
                18,
                17,
                0b001,
            ));
        }
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u32; 4]; 10] = [
        [0, 1, 0xeeee_eeee, 0xeeee_eeee],
        [0x1111_1111, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee],
        [0x3333_3333, 0x4444_4444, 0xeeee_eeee, 0xeeee_eeee],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0, 0, 0xeeee_eeee, 0xeeee_eeee],
        [0, 0, 0xeeee_eeee, 0xeeee_eeee],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0x5454_5454],
        [0xa1a2_a3a4, 0x2222_2222, 0xeeee_eeee, 0xeeee_eeee],
        [0xb1b2_b3b4, 0x4444_4444, 0xeeee_eeee, 0xeeee_eeee],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0x5353_5353, 0x5454_5454],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_segment_e16_m1_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const BLOCK_BYTES: i32 = 16;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_FIELD0_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const INITIAL_FIELD1_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const SOURCE_SEGMENT_OFFSET_BYTES: i32 = BLOCK_BYTES * 3;
    const LOAD_FIELD0_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const LOAD_FIELD1_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 5;
    const STORE_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 6;
    const EXPECTED_FIELD0_OFFSET_BYTES: i32 = BLOCK_BYTES * 7;
    const EXPECTED_FIELD1_OFFSET_BYTES: i32 = BLOCK_BYTES * 8;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 9;
    const FAIL_INSTRUCTION_INDEX: i32 = 47;

    let mut words = vec![
        u_type(0, 10, 0x17),                                           // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),                // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),                // addi x12, x10, mask data
        i_type(INITIAL_FIELD0_OFFSET_BYTES, 10, 0b000, 13, 0x13), // addi x13, x10, initial field 0
        i_type(INITIAL_FIELD1_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial field 1
        i_type(SOURCE_SEGMENT_OFFSET_BYTES, 10, 0b000, 15, 0x13), // addi x15, x10, source segment
        i_type(LOAD_FIELD0_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load field 0 result
        i_type(LOAD_FIELD1_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, load field 1 result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 20, 0x13),       // addi x20, x10, store result
        i_type(EXPECTED_FIELD0_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected field 0
        i_type(EXPECTED_FIELD1_OFFSET_BYTES, 10, 0b000, 22, 0x13), // addi x22, x10, expected field 1
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 23, 0x13),  // addi x23, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                             // addi x11, x0, vl
        vsetvli_type(0xc8, 11, 5), // vsetvli x5, x11, e16, m1, ta, ma
        vector_unit_stride_load_type(true, 0b101, 12, 8), // vle16.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b101, 13, 2), // vle16.v v2, (x13)
        vector_unit_stride_load_type(true, 0b101, 14, 3), // vle16.v v3, (x14)
        vector_unit_stride_segment_load_type(false, 2, 0b101, 15, 2), // vlseg2e16.v v2, (x15), v0.t
        vector_unit_stride_store_type(true, 0b101, 16, 2), // vse16.v v2, (x16)
        vector_unit_stride_store_type(true, 0b101, 19, 3), // vse16.v v3, (x19)
        vector_unit_stride_segment_store_type(false, 2, 0b101, 20, 2), // vsseg2e16.v v2, (x20), v0.t
    ];

    for (base, expected_base, halfwords_to_compare) in [(16, 21, 2), (19, 22, 2), (20, 23, 4)] {
        for halfword_index in 0..halfwords_to_compare {
            let offset = (halfword_index * 2) as i32;
            words.push(i_type(offset, base, 0b101, 17, 0x03)); // lhu x17, observed halfword
            words.push(i_type(offset, expected_base, 0b101, 18, 0x03)); // lhu x18, expected halfword
            let branch_index = words.len() as i32;
            words.push(b_type(
                (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
                18,
                17,
                0b001,
            ));
        }
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u16; 8]; 10] = [
        [0, 1, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee],
        [
            0x1111, 0x2222, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
        [
            0x3333, 0x4444, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
        [
            0xa1a2, 0xb1b2, 0xc1c2, 0xd1d2, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
        [0, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee],
        [0, 0, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee],
        [
            0x5151, 0x5252, 0x5353, 0x5454, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
        [
            0xa1a2, 0x2222, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
        [
            0xb1b2, 0x4444, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
        [
            0xa1a2, 0xb1b2, 0x5353, 0x5454, 0xeeee, 0xeeee, 0xeeee, 0xeeee,
        ],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        program.extend_from_slice(&u16_halfwords_to_bytes(&block));
    }
    program
}

fn masked_unit_stride_segment_e64_m1_two_line_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 512;
    const BLOCK_BYTES: i32 = 32;
    const MASK_OFFSET_BYTES: i32 = 0;
    const INITIAL_FIELD0_OFFSET_BYTES: i32 = BLOCK_BYTES;
    const INITIAL_FIELD1_OFFSET_BYTES: i32 = BLOCK_BYTES * 2;
    const SOURCE_SEGMENT_OFFSET_BYTES: i32 = BLOCK_BYTES * 3;
    const LOAD_FIELD0_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 4;
    const LOAD_FIELD1_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 5;
    const STORE_RESULT_OFFSET_BYTES: i32 = BLOCK_BYTES * 6;
    const EXPECTED_FIELD0_OFFSET_BYTES: i32 = BLOCK_BYTES * 7;
    const EXPECTED_FIELD1_OFFSET_BYTES: i32 = BLOCK_BYTES * 8;
    const EXPECTED_STORE_OFFSET_BYTES: i32 = BLOCK_BYTES * 9;
    const FAIL_INSTRUCTION_INDEX: i32 = 47;

    let mut words = vec![
        u_type(0, 10, 0x17),                                           // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13),                // addi x10, x10, data
        i_type(MASK_OFFSET_BYTES, 10, 0b000, 12, 0x13),                // addi x12, x10, mask data
        i_type(INITIAL_FIELD0_OFFSET_BYTES, 10, 0b000, 13, 0x13), // addi x13, x10, initial field 0
        i_type(INITIAL_FIELD1_OFFSET_BYTES, 10, 0b000, 14, 0x13), // addi x14, x10, initial field 1
        i_type(SOURCE_SEGMENT_OFFSET_BYTES, 10, 0b000, 15, 0x13), // addi x15, x10, source segment
        i_type(LOAD_FIELD0_RESULT_OFFSET_BYTES, 10, 0b000, 16, 0x13), // addi x16, x10, load field 0 result
        i_type(LOAD_FIELD1_RESULT_OFFSET_BYTES, 10, 0b000, 19, 0x13), // addi x19, x10, load field 1 result
        i_type(STORE_RESULT_OFFSET_BYTES, 10, 0b000, 20, 0x13),       // addi x20, x10, store result
        i_type(EXPECTED_FIELD0_OFFSET_BYTES, 10, 0b000, 21, 0x13), // addi x21, x10, expected field 0
        i_type(EXPECTED_FIELD1_OFFSET_BYTES, 10, 0b000, 22, 0x13), // addi x22, x10, expected field 1
        i_type(EXPECTED_STORE_OFFSET_BYTES, 10, 0b000, 23, 0x13),  // addi x23, x10, expected store
        i_type(2, 0, 0b000, 11, 0x13),                             // addi x11, x0, vl
        vsetvli_type(0xd8, 11, 5), // vsetvli x5, x11, e64, m1, ta, ma
        vector_unit_stride_load_type(true, 0b111, 12, 8), // vle64.v v8, (x12)
        vector_vi_type(0b011000, 8, 0, 0), // vmseq.vi v0, v8, 0
        vector_unit_stride_load_type(true, 0b111, 13, 2), // vle64.v v2, (x13)
        vector_unit_stride_load_type(true, 0b111, 14, 3), // vle64.v v3, (x14)
        vector_unit_stride_segment_load_type(false, 2, 0b111, 15, 2), // vlseg2e64.v v2, (x15), v0.t
        vector_unit_stride_store_type(true, 0b111, 16, 2), // vse64.v v2, (x16)
        vector_unit_stride_store_type(true, 0b111, 19, 3), // vse64.v v3, (x19)
        vector_unit_stride_segment_store_type(false, 2, 0b111, 20, 2), // vsseg2e64.v v2, (x20), v0.t
    ];

    for (base, expected_base, words_to_compare) in [(16, 21, 2), (19, 22, 2), (20, 23, 4)] {
        for word_index in 0..words_to_compare {
            let offset = (word_index * 8) as i32;
            words.push(i_type(offset, base, 0b011, 17, 0x03)); // ld x17, observed doubleword
            words.push(i_type(offset, expected_base, 0b011, 18, 0x03)); // ld x18, expected doubleword
            let branch_index = words.len() as i32;
            words.push(b_type(
                (FAIL_INSTRUCTION_INDEX - branch_index) * 4,
                18,
                17,
                0b001,
            ));
        }
    }

    words.push(0x0000_0073); // ecall
    words.push(0x0000_0000); // fail: invalid instruction
    assert_eq!(words.len() as i32, FAIL_INSTRUCTION_INDEX + 1);
    assert!(words.len() * 4 <= DATA_OFFSET_BYTES as usize);
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let blocks: [[u64; 4]; 10] = [
        [0, 1, 0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee],
        [
            0x1111_2222_3333_4444,
            0x5555_6666_7777_8888,
            0xeeee_eeee_eeee_eeee,
            0xeeee_eeee_eeee_eeee,
        ],
        [
            0x9999_aaaa_bbbb_cccc,
            0xdddd_eeee_ffff_0001,
            0xeeee_eeee_eeee_eeee,
            0xeeee_eeee_eeee_eeee,
        ],
        [
            0xa1a2_a3a4_a5a6_a7a8,
            0xb1b2_b3b4_b5b6_b7b8,
            0xc1c2_c3c4_c5c6_c7c8,
            0xd1d2_d3d4_d5d6_d7d8,
        ],
        [0, 0, 0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee],
        [0, 0, 0xeeee_eeee_eeee_eeee, 0xeeee_eeee_eeee_eeee],
        [
            0x5151_5151_5151_5151,
            0x5252_5252_5252_5252,
            0x5353_5353_5353_5353,
            0x5454_5454_5454_5454,
        ],
        [
            0xa1a2_a3a4_a5a6_a7a8,
            0x5555_6666_7777_8888,
            0xeeee_eeee_eeee_eeee,
            0xeeee_eeee_eeee_eeee,
        ],
        [
            0xb1b2_b3b4_b5b6_b7b8,
            0xdddd_eeee_ffff_0001,
            0xeeee_eeee_eeee_eeee,
            0xeeee_eeee_eeee_eeee,
        ],
        [
            0xa1a2_a3a4_a5a6_a7a8,
            0xb1b2_b3b4_b5b6_b7b8,
            0x5353_5353_5353_5353,
            0x5454_5454_5454_5454,
        ],
    ];
    let mut program = riscv64_program(&words);
    for block in blocks {
        for word in block {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn vector_unit_stride_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_fault_only_load_type(vm_unmasked: bool, width: u32, rs1: u8, vd: u8) -> u32 {
    (0b10000 << 20)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_store_type(vm_unmasked: bool, width: u32, rs1: u8, vs3: u8) -> u32 {
    (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn vector_unit_stride_segment_load_type(
    vm_unmasked: bool,
    fields: u32,
    width: u32,
    rs1: u8,
    vd: u8,
) -> u32 {
    ((fields - 1) << 29)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_unit_stride_segment_store_type(
    vm_unmasked: bool,
    fields: u32,
    width: u32,
    rs1: u8,
    vs3: u8,
) -> u32 {
    ((fields - 1) << 29)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn vector_strided_load_type(vm_unmasked: bool, width: u32, rs1: u8, rs2: u8, vd: u8) -> u32 {
    (0b10 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_strided_store_type(vm_unmasked: bool, width: u32, rs1: u8, rs2: u8, vs3: u8) -> u32 {
    (0b10 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn vector_indexed_unordered_load_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs2: u8,
    vd: u8,
) -> u32 {
    (0b01 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vd) << 7)
        | 0x07
}

fn vector_indexed_unordered_store_type(
    vm_unmasked: bool,
    width: u32,
    rs1: u8,
    vs2: u8,
    vs3: u8,
) -> u32 {
    (0b01 << 26)
        | (u32::from(vm_unmasked) << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (width << 12)
        | (u32::from(vs3) << 7)
        | 0x27
}

fn fp_r_type(funct7: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8) -> u32 {
    (funct7 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | 0x53
}

fn fp_r4_type(rs3: u8, funct2: u32, rs2: u8, rs1: u8, funct3: u32, rd: u8, opcode: u32) -> u32 {
    (u32::from(rs3) << 27)
        | (funct2 << 25)
        | (u32::from(rs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(rd) << 7)
        | opcode
}

fn vsetvli_type(vtype: u32, rs1: u8, rd: u8) -> u32 {
    (vtype << 20) | (u32::from(rs1) << 15) | (0b111 << 12) | (u32::from(rd) << 7) | 0x57
}

fn vector_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_mvv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(vs1) << 15)
        | (0b010 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_mvx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b110 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_reduction_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(funct6, vs2, vs1, vd)
}

fn vector_widening_reduction_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_vv_type(funct6, vs2, vs1, vd)
}

fn vector_widening_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_mvv_type(funct6, vs2, vs1, vd)
}

fn vector_widening_vx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    vector_mvx_type(funct6, vs2, rs1, vd)
}

fn vector_vi_type(funct6: u32, vs2: u8, imm: i8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(imm as u8 & 0x1f) << 15)
        | (0b011 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_fp_latency_program(instruction: u32) -> [u32; 8] {
    [
        u_type(0x3f80_0000, 8, 0x37),  // lui x8, 0x3f800 (f32 1.0 bits)
        fp_r_type(0x78, 0, 8, 0x0, 8), // fmv.w.x f8, x8
        0x0020_0513,                   // addi x10, x0, 2
        vsetvli_type(0xd0, 10, 5),     // vsetvli x5, x10, e32, m1, ta, ma
        vmv_v_x_type(8, 1),            // vmv.v.x v1, x8
        vmv_v_x_type(8, 2),            // vmv.v.x v2, x8
        instruction,
        0x0000_0073, // ecall
    ]
}

fn vmv_v_x_type(rs1: u8, vd: u8) -> u32 {
    vector_vx_type(0b010111, 0, rs1, vd)
}

fn vector_vx_type(funct6: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (0b100 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_float_vv_type(funct6: u32, vs2: u8, vs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b001, vs2, vs1, vd)
}

fn vector_float_vf_type(funct6: u32, vs2: u8, fs1: u8, vd: u8) -> u32 {
    vector_float_type(funct6, 0b101, vs2, fs1, vd)
}

fn vector_float_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (1 << 25)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn vector_float_masked_type(funct6: u32, funct3: u32, vs2: u8, rs1: u8, vd: u8) -> u32 {
    (funct6 << 26)
        | (u32::from(vs2) << 20)
        | (u32::from(rs1) << 15)
        | (funct3 << 12)
        | (u32::from(vd) << 7)
        | 0x57
}

fn in_order_pipeline_stats_for_width_and_memory(
    path: &std::path::Path,
    width: u64,
    memory_system: &str,
    max_tick: u64,
) -> String {
    let width = width.to_string();
    let max_tick = max_tick.to_string();
    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            &max_tick,
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            memory_system,
            "--riscv-branch-lookahead",
            "2",
            "--riscv-in-order-width",
            &width,
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"committed_instructions\":5"));
    stdout
}

#[test]
fn rem6_run_stats_include_issued_fetch_ahead_before_response() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-issued-fetch-ahead-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "16",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert_eq!(json_u64_field(&stdout, "\"committed_instructions\":"), 1);
    assert_eq!(json_u64_field(&stdout, "\"in_flight\":"), 1);
    assert!(stdout.contains("\"stage_in_flight\":{\"fetch1\":"));
    let advanced = json_u64_field(&stdout, "\"advanced\":");
    let retired = json_u64_field(&stdout, "\"retired\":");
    assert!(
        advanced > retired,
        "pipeline advance history should include non-retire cycles: {stdout}"
    );
    assert_stat_greater_than(
        &stdout,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        0,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.retired",
        "Count",
        1,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.advanced",
        "Count",
        advanced,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.cpu0.pipeline.in_order.in_flight",
        "Count",
        1,
        "constant",
    );
    let stage_in_flight = [
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.fetch1.in_flight"),
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.fetch2.in_flight"),
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.decode.in_flight"),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.execute.in_flight",
        ),
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stage.commit.in_flight"),
    ];
    assert_eq!(stage_in_flight.iter().sum::<u64>(), 1);
    assert!(stage_in_flight.contains(&1));
    let stage_max_in_flight = [
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.fetch1.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.fetch2.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.decode.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.execute.max_in_flight",
        ),
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.stage.commit.max_in_flight",
        ),
    ];
    assert!(stage_max_in_flight
        .iter()
        .zip(stage_in_flight)
        .all(|(max, current)| *max >= current));
    assert!(stage_max_in_flight.iter().sum::<u64>() >= 1);
    assert_stat(
        &stdout,
        "sim.memory.fetch.requests",
        "Count",
        2,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "sim.memory.fetch.responses",
        "Count",
        1,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_issue_jal_fetch_ahead_before_retire() {
    let program = riscv64_program(&[
        0x0070_0293,  // addi x5, x0, 7
        j_type(8, 0), // jal x0, target
        0x0010_0313,  // addi x6, x0, 1
        0x0000_0073,  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-jal-fetch-ahead-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "28",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("\"status\":\"stopped_at_tick_limit\""));
    assert_eq!(
        json_u64_field(&stdout, "\"committed_instructions\":"),
        2,
        "{stdout}"
    );
    assert_eq!(json_u64_field(&stdout, "\"in_flight\":"), 1, "{stdout}");
    assert_eq!(
        stat_value(&stdout, "sim.memory.fetch.requests"),
        3,
        "{stdout}"
    );
    assert_eq!(
        stat_value(&stdout, "sim.memory.fetch.responses"),
        2,
        "{stdout}"
    );
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
}

#[test]
fn rem6_run_stats_emit_in_order_fetch_wait_cycles_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-fetch-wait-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
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
    let fetch_wait_cycles = json_u64_field(&stdout, "\"fetch_wait_cycles\":");
    assert!(fetch_wait_cycles > 0);
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.fetch_wait_cycles"),
        fetch_wait_cycles
    );
    assert_stat_greater_than(
        &stdout,
        "sim.cpu0.pipeline.in_order.fetch_wait_cycles",
        "Cycle",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_stats_emit_in_order_resource_stalls_for_pending_parallel_fetch() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-pending-fetch-resource-stalls", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
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
    let fetch_wait_cycles = json_u64_field(&stdout, "\"fetch_wait_cycles\":");
    let stall_cycles = json_u64_field(&stdout, "\"stall_cycles\":");
    let resource_blocked = json_u64_field(&stdout, "\"resource_blocked\":");
    let ordering_blocked = json_u64_field(&stdout, "\"ordering_blocked\":");
    let stage_resource_blocked = json_stage_summary(&stdout, "\"stage_resource_blocked\":{");
    let stage_ordering_blocked = json_stage_summary(&stdout, "\"stage_ordering_blocked\":{");
    let stage_resource_blocked_cycles =
        json_stage_summary(&stdout, "\"stage_resource_blocked_cycles\":{");
    let stage_ordering_blocked_cycles =
        json_stage_summary(&stdout, "\"stage_ordering_blocked_cycles\":{");
    assert!(fetch_wait_cycles > 0, "{stdout}");
    assert!(stall_cycles > 0, "{stdout}");
    assert!(resource_blocked > 0, "{stdout}");
    assert_eq!(stage_resource_blocked.iter().sum::<u64>(), resource_blocked);
    assert_eq!(stage_ordering_blocked.iter().sum::<u64>(), ordering_blocked);
    assert!(
        stage_resource_blocked.iter().any(|value| *value > 0),
        "{stdout}"
    );
    assert!(
        stage_resource_blocked_cycles.iter().any(|value| *value > 0),
        "{stdout}"
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.stall_cycles"),
        stall_cycles
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.resource_blocked"),
        resource_blocked
    );
    for (index, stage) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.resource_blocked")
            ),
            stage_resource_blocked[index]
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.ordering_blocked")
            ),
            stage_ordering_blocked[index]
        );
        assert!(
            stage_resource_blocked_cycles[index] <= stage_resource_blocked[index],
            "{stdout}"
        );
        assert!(
            stage_ordering_blocked_cycles[index] <= stage_ordering_blocked[index],
            "{stdout}"
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.resource_blocked_cycles")
            ),
            stage_resource_blocked_cycles[index]
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.ordering_blocked_cycles")
            ),
            stage_ordering_blocked_cycles[index]
        );
    }
}

#[test]
fn rem6_run_text_stats_emit_in_order_resource_stall_aliases_from_pending_parallel_fetch() {
    let program = riscv64_program(&[
        0x0070_0293, // addi x5, x0, 7
        0x0000_0073, // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-resource-stall-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
            "--min-remote-delay",
            "2",
            "--memory-route-delay",
            "5",
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
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.stallCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.stall_cycles")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.fetchWaitCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.fetch_wait_cycles")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.executeWaitCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.execute_wait_cycles")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.resourceBlocked"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.resource_blocked")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.orderingBlocked"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.ordering_blocked")
    );
    assert!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.resourceBlocked") > 0,
        "{stdout}"
    );
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.resourceBlocked")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.resource_blocked")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.resourceBlockedCycles")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.resource_blocked_cycles")
            )
        );
    }
    assert!(
        text_stat_line(&stdout, "system.cpu.pipeline.inOrder.stallCycles").contains("unit=Cycle"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.pipeline.inOrder.executeWaitCycles")
            .contains("unit=Cycle"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.pipeline.inOrder.resourceBlocked")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.pipeline.inOrder.stage.fetch1.resourceBlockedCycles"
        )
        .contains("unit=Cycle"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_text_stats_emit_in_order_stall_cause_stage_aliases() {
    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),          // auipc x2, 0
        i_type(24, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),  // ld x5, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),        // sd x6, 8(x2)
        0x0000_0073,                 // ecall
    ]);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&[0; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-stall-cause-stage-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "240",
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

    for (cause, alias_cause) in [
        ("fetch_wait", "fetchWait"),
        ("data_wait", "dataWait"),
        ("execute_wait", "executeWait"),
    ] {
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stallCause.{alias_cause}.records")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stall_cause.{cause}.records")
            )
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stallCause.{alias_cause}.records")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
        for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
            assert_eq!(
                text_stat_value(
                    &stdout,
                    &format!(
                        "system.cpu.pipeline.inOrder.stallCause.{alias_cause}.stage.{stage}.resourceBlocked"
                    )
                ),
                text_stat_value(
                    &stdout,
                    &format!(
                        "sim.cpu0.pipeline.in_order.stall_cause.{cause}.stage.{stage}.resource_blocked"
                    )
                )
            );
            assert_eq!(
                text_stat_value(
                    &stdout,
                    &format!(
                        "system.cpu.pipeline.inOrder.stallCause.{alias_cause}.stage.{stage}.resourceBlockedCycles"
                    )
                ),
                text_stat_value(
                    &stdout,
                    &format!(
                        "sim.cpu0.pipeline.in_order.stall_cause.{cause}.stage.{stage}.resource_blocked_cycles"
                    )
                )
            );
        }
    }
    assert!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.stallCause.dataWait.stage.commit.resourceBlockedCycles"
        ) > 0,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.stallCause.executeWait.stage.execute.resourceBlockedCycles"
        ),
        0
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.pipeline.inOrder.stallCause.dataWait.stage.commit.resourceBlocked"
        )
        .contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.pipeline.inOrder.stallCause.dataWait.stage.commit.resourceBlockedCycles"
        )
        .contains("unit=Cycle"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_text_stats_emit_in_order_stage_movement_aliases() {
    let program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13),  // addi x5, x0, 1
        i_type(2, 0, 0x0, 6, 0x13),  // addi x6, x0, 2
        i_type(3, 0, 0x0, 7, 0x13),  // addi x7, x0, 3
        i_type(4, 0, 0x0, 28, 0x13), // addi x28, x0, 4
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-stage-movement-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-in-order-width",
            "3",
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
        text_stat_value(&stdout, "sim.cpu0.instructions.committed"),
        5
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionRedirects"
        ),
        0
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionFlushes"
        ),
        0
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionFlushCycles"
        ),
        0
    );

    let mut stage_advanced = [0_u64; 5];
    let mut stage_advanced_cycles = [0_u64; 5];
    let mut stage_retired = [0_u64; 5];
    let mut stage_retired_cycles = [0_u64; 5];
    for (index, stage) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .into_iter()
        .enumerate()
    {
        for (source_name, alias_name, unit) in [
            ("width", "width", "Count"),
            ("in_flight", "inFlight", "Count"),
            ("max_in_flight", "maxInFlight", "Count"),
            ("advanced", "advanced", "Count"),
            ("advanced_cycles", "advancedCycles", "Cycle"),
            ("retired", "retired", "Count"),
            ("retired_cycles", "retiredCycles", "Cycle"),
        ] {
            let alias = format!("system.cpu.pipeline.inOrder.stage.{stage}.{alias_name}");
            let source = format!("sim.cpu0.pipeline.in_order.stage.{stage}.{source_name}");
            assert_eq!(
                text_stat_value(&stdout, &alias),
                text_stat_value(&stdout, &source)
            );
            let alias_line = text_stat_line(&stdout, &alias);
            let source_line = text_stat_line(&stdout, &source);
            assert_eq!(text_stat_tag(alias_line, "unit="), unit, "{alias}");
            assert_eq!(
                text_stat_tag(alias_line, "unit="),
                text_stat_tag(source_line, "unit="),
                "{alias}"
            );
            assert_eq!(
                text_stat_tag(alias_line, "reset_policy="),
                text_stat_tag(source_line, "reset_policy="),
                "{alias}"
            );
        }
        for branch_alias in [
            format!("system.cpu.pipeline.inOrder.stage.{stage}.branchPredictionFlushed"),
            format!("system.cpu.pipeline.inOrder.stage.{stage}.branchPredictionFlushedCycles"),
            format!(
                "system.cpu.pipeline.inOrder.flushCause.branchPrediction.stage.{stage}.flushed"
            ),
            format!(
                "system.cpu.pipeline.inOrder.flushCause.branchPrediction.stage.{stage}.flushedCycles"
            ),
            format!(
                "system.cpu.pipeline.inOrder.redirectCause.branchPrediction.stage.{stage}.flushed"
            ),
            format!(
                "system.cpu.pipeline.inOrder.redirectCause.branchPrediction.stage.{stage}.flushedCycles"
            ),
        ] {
            assert_eq!(
                text_stat_value(&stdout, &branch_alias),
                0,
                "straight-line movement row should not emit branch-prediction alias {branch_alias}\n{stdout}"
            );
        }
        stage_advanced[index] = text_stat_value(
            &stdout,
            &format!("system.cpu.pipeline.inOrder.stage.{stage}.advanced"),
        );
        stage_advanced_cycles[index] = text_stat_value(
            &stdout,
            &format!("system.cpu.pipeline.inOrder.stage.{stage}.advancedCycles"),
        );
        stage_retired[index] = text_stat_value(
            &stdout,
            &format!("system.cpu.pipeline.inOrder.stage.{stage}.retired"),
        );
        stage_retired_cycles[index] = text_stat_value(
            &stdout,
            &format!("system.cpu.pipeline.inOrder.stage.{stage}.retiredCycles"),
        );
    }

    assert_eq!(
        stage_advanced.iter().sum::<u64>(),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.advanced")
    );
    assert_eq!(
        stage_retired.iter().sum::<u64>(),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.retired")
    );
    assert!(stage_advanced_cycles.iter().any(|cycles| *cycles > 0));
    assert!(stage_retired_cycles.iter().any(|cycles| *cycles > 0));
    assert!(
        stage_advanced
            .iter()
            .zip(stage_advanced_cycles.iter())
            .any(|(advanced, cycles)| advanced > cycles),
        "widened pipeline should distinguish movement counts from cycle presence: advanced={stage_advanced:?} cycles={stage_advanced_cycles:?}"
    );
    assert!(
        stage_retired[4] > 0,
        "retirement movement should be attributed to commit stage: {stage_retired:?}"
    );
}

#[test]
fn rem6_run_stats_emit_in_order_branch_redirects_from_execution() {
    let program = riscv64_program(&[
        0x0070_0293,          // addi x5, x0, 7
        b_type(8, 0, 0, 0x0), // beq x0, x0, target
        0x0010_0313,          // addi x6, x0, 1
        0x0000_0073,          // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-redirect-stats", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
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
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let branch_predictions = json_u64_field(&stdout, "\"branch_predictions\":");
    let branch_mispredictions = json_u64_field(&stdout, "\"branch_mispredictions\":");
    let conditional_branch_predictions =
        json_u64_field(&stdout, "\"conditional_branch_predictions\":");
    let conditional_branch_predicted_taken =
        json_u64_field(&stdout, "\"conditional_branch_predicted_taken\":");
    let conditional_branch_mispredictions =
        json_u64_field(&stdout, "\"conditional_branch_mispredictions\":");
    let advanced = json_u64_field(&stdout, "\"advanced\":");
    let flushed = json_u64_field(&stdout, "\"flushed\":");
    let flush_cycles = json_u64_field(&stdout, "\"flush_cycles\":");
    let resource_blocked = json_u64_field(&stdout, "\"resource_blocked\":");
    let ordering_blocked = json_u64_field(&stdout, "\"ordering_blocked\":");
    let branch_prediction_flushes = json_u64_field(&stdout, "\"branch_prediction_flushes\":");
    let branch_prediction_flush_cycles =
        json_u64_field(&stdout, "\"branch_prediction_flush_cycles\":");
    let redirects = json_u64_field(&stdout, "\"redirects\":");
    let stage_flushed = json_stage_summary(&stdout, "\"stage_flushed\":{");
    let stage_flushed_cycles = json_stage_summary(&stdout, "\"stage_flushed_cycles\":{");
    let stage_branch_prediction_flushed =
        json_stage_summary(&stdout, "\"stage_branch_prediction_flushed\":{");
    let stage_branch_prediction_flushed_cycles =
        json_stage_summary(&stdout, "\"stage_branch_prediction_flushed_cycles\":{");

    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.branch_predictions"),
        branch_predictions
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.branch_mispredictions"),
        branch_mispredictions
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_predictions"
        ),
        conditional_branch_predictions
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_predicted_taken"
        ),
        conditional_branch_predicted_taken
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_mispredictions"
        ),
        conditional_branch_mispredictions
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.advanced"),
        advanced
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.flushed"),
        flushed
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.flush_cycles"),
        flush_cycles
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.resource_blocked"),
        resource_blocked
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.ordering_blocked"),
        ordering_blocked
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_flushes"
        ),
        branch_prediction_flushes
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_flush_cycles"
        ),
        branch_prediction_flush_cycles
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.redirects"),
        redirects
    );
    assert!(branch_predictions > 0);
    assert!(branch_mispredictions > 0);
    assert_eq!(conditional_branch_predictions, branch_predictions);
    assert!(conditional_branch_predicted_taken <= conditional_branch_predictions);
    assert_eq!(conditional_branch_mispredictions, branch_mispredictions);
    assert!(advanced > 0);
    assert!(flushed > 0);
    assert!(flush_cycles > 0);
    assert!(flush_cycles <= flushed);
    assert!(flushed >= branch_prediction_flushes);
    assert!(branch_prediction_flushes > 0);
    assert!(branch_prediction_flush_cycles > 0);
    assert!(branch_prediction_flush_cycles <= branch_prediction_flushes);
    assert!(flush_cycles >= branch_prediction_flush_cycles);
    assert!(redirects > 0);
    assert_eq!(stage_flushed.iter().sum::<u64>(), flushed);
    assert!(stage_flushed.iter().any(|value| *value > 0), "{stdout}");
    assert!(
        stage_flushed_cycles.iter().any(|value| *value > 0),
        "{stdout}"
    );
    assert!(stage_flushed_cycles.iter().sum::<u64>() >= flush_cycles);
    assert!(stage_flushed_cycles.iter().sum::<u64>() <= flushed);
    assert_eq!(
        stage_branch_prediction_flushed.iter().sum::<u64>(),
        branch_prediction_flushes
    );
    assert!(
        stage_branch_prediction_flushed
            .iter()
            .any(|value| *value > 0),
        "{stdout}"
    );
    assert!(
        stage_branch_prediction_flushed_cycles
            .iter()
            .any(|value| *value > 0),
        "{stdout}"
    );
    assert!(
        stage_branch_prediction_flushed_cycles.iter().sum::<u64>()
            >= branch_prediction_flush_cycles
    );
    assert!(
        stage_branch_prediction_flushed_cycles.iter().sum::<u64>() <= branch_prediction_flushes
    );
    for (index, stage) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.flushed")
            ),
            stage_flushed[index]
        );
        assert!(
            stage_flushed_cycles[index] <= stage_flushed[index],
            "{stage} flush cycle count should not exceed flushed instructions\n{stdout}"
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.flushed_cycles")
            ),
            stage_flushed_cycles[index]
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.branch_prediction_flushed")
            ),
            stage_branch_prediction_flushed[index]
        );
        assert!(
            stage_branch_prediction_flushed_cycles[index]
                <= stage_branch_prediction_flushed[index],
            "{stage} branch-prediction flush cycle count should not exceed flushed instructions\n{stdout}"
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!(
                    "sim.cpu0.pipeline.in_order.stage.{stage}.branch_prediction_flushed_cycles"
                )
            ),
            stage_branch_prediction_flushed_cycles[index]
        );
    }
    assert!(stdout.contains("\"x5\":\"0x7\""));
    assert!(!stdout.contains("\"x6\":\"0x1\""));
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
    ] {
        assert_json_stat_alias(&json, source, alias);
    }
    assert_json_stat_absent(&json, "system.cpu0.branchPred.condPredicted");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.condPredictedTaken");
    assert_json_stat_absent(&json, "system.cpu0.branchPred.condIncorrect");
}

#[test]
fn rem6_run_text_stats_emit_in_order_branch_flush_aliases_from_redirect() {
    let program = riscv64_program(&[
        0x0070_0293,          // addi x5, x0, 7
        b_type(8, 0, 0, 0x0), // beq x0, x0, target
        0x0010_0313,          // addi x6, x0, 1
        0x0000_0073,          // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-flush-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
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
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.flushed"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.flushed")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.flushCycles"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.flush_cycles")
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionFlushes"
        ),
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_flushes"
        )
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionFlushCycles"
        ),
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_flush_cycles"
        )
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.redirects"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.redirects")
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionRedirects"
        ),
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_redirects"
        )
    );
    assert!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionRedirects"
        ) > 0,
        "{stdout}"
    );
    assert!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionFlushes"
        ) > 0,
        "{stdout}"
    );
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.flushed")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.flushed")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.branchPredictionFlushed")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.branch_prediction_flushed")
            )
        );
    }
    assert!(
        text_stat_line(&stdout, "system.cpu.pipeline.inOrder.flushCycles").contains("unit=Cycle"),
        "{stdout}"
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionFlushes"
        )
        .contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_text_stats_emit_in_order_flush_redirect_cause_stage_aliases() {
    let branch_program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        b_type(8, 5, 5, 0x0),       // beq x5, x5, target
        i_type(9, 0, 0x0, 6, 0x13), // addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // target: addi x7, x0, 7
        0x0000_0073,                // ecall
    ]);
    let branch_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &branch_program);
    let branch_path = temp_binary("in-order-branch-cause-stage-text-aliases", &branch_elf);

    let branch_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            branch_path.to_str().unwrap(),
            "--max-tick",
            "160",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-branch-lookahead",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        branch_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&branch_output.stderr)
    );
    let branch_stdout = String::from_utf8(branch_output.stdout).unwrap();
    assert!(
        text_stat_value(
            &branch_stdout,
            "system.cpu.pipeline.inOrder.branchPredictionRedirects"
        ) > 0,
        "{branch_stdout}"
    );
    let (branch_flushed, branch_flushed_cycles) = assert_in_order_cause_stage_aliases(
        &branch_stdout,
        "flush_cause",
        "flushCause",
        "branch_prediction",
        "branchPrediction",
    );
    assert!(
        branch_flushed.iter().any(|flushed| *flushed > 0),
        "branch run should expose nonzero branch-prediction flush-cause aliases: {branch_flushed:?}"
    );
    assert!(
        branch_flushed_cycles.iter().any(|cycles| *cycles > 0),
        "branch run should expose nonzero branch-prediction flush-cause alias cycles: {branch_flushed_cycles:?}"
    );
    assert_eq!(
        assert_in_order_cause_stage_aliases(
            &branch_stdout,
            "redirect_cause",
            "redirectCause",
            "branch_prediction",
            "branchPrediction",
        ),
        (branch_flushed, branch_flushed_cycles)
    );
    let branch_trap_flushed = ([0, 0, 1, 0, 0], [0, 0, 1, 0, 0]);
    assert_eq!(
        assert_in_order_cause_stage_aliases(
            &branch_stdout,
            "flush_cause",
            "flushCause",
            "trap_redirect",
            "trapRedirect",
        ),
        branch_trap_flushed
    );
    assert_eq!(
        assert_in_order_cause_stage_aliases(
            &branch_stdout,
            "redirect_cause",
            "redirectCause",
            "trap_redirect",
            "trapRedirect",
        ),
        branch_trap_flushed
    );

    let trap_program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        i_type(9, 0, 0x0, 6, 0x13), // wrong-path addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let trap_elf = riscv64_elf(0x8000_0000, 0x8000_0000, &trap_program);
    let trap_path = temp_binary("in-order-trap-cause-stage-text-aliases", &trap_elf);

    let trap_output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            trap_path.to_str().unwrap(),
            "--max-tick",
            "120",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-in-order-width",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        trap_output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&trap_output.stderr)
    );
    let trap_stdout = String::from_utf8(trap_output.stdout).unwrap();
    assert_eq!(
        text_stat_value(&trap_stdout, "system.cpu.pipeline.inOrder.trapRedirects"),
        1
    );
    let (trap_flushed, trap_flushed_cycles) = assert_in_order_cause_stage_aliases(
        &trap_stdout,
        "flush_cause",
        "flushCause",
        "trap_redirect",
        "trapRedirect",
    );
    assert!(
        trap_flushed.iter().any(|flushed| *flushed > 0),
        "trap run should expose nonzero trap-redirect flush-cause aliases: {trap_flushed:?}"
    );
    assert!(
        trap_flushed_cycles.iter().any(|cycles| *cycles > 0),
        "trap run should expose nonzero trap-redirect flush-cause alias cycles: {trap_flushed_cycles:?}"
    );
    assert_eq!(
        assert_in_order_cause_stage_aliases(
            &trap_stdout,
            "redirect_cause",
            "redirectCause",
            "trap_redirect",
            "trapRedirect",
        ),
        (trap_flushed, trap_flushed_cycles)
    );
    assert_eq!(
        assert_in_order_cause_stage_aliases(
            &trap_stdout,
            "flush_cause",
            "flushCause",
            "branch_prediction",
            "branchPrediction",
        ),
        ([0; 5], [0; 5])
    );
    assert_eq!(
        assert_in_order_cause_stage_aliases(
            &trap_stdout,
            "redirect_cause",
            "redirectCause",
            "branch_prediction",
            "branchPrediction",
        ),
        ([0; 5], [0; 5])
    );
}

#[test]
fn rem6_run_stats_emit_in_order_trap_redirect_flushes_from_execution() {
    let program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        i_type(9, 0, 0x0, 6, 0x13), // wrong-path addi x6, x0, 9
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-trap-redirect-flush-stats", &elf);

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
            "5",
            "--stats-format",
            "json",
            "--execute",
            "--memory-system",
            "direct",
            "--riscv-in-order-width",
            "2",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let json: Value = serde_json::from_str(&stdout).unwrap();
    let branch_prediction_redirects = json_u64_field(&stdout, "\"branch_prediction_redirects\":");
    let trap_redirects = json_u64_field(&stdout, "\"trap_redirects\":");
    let trap_redirect_flushes = json_u64_field(&stdout, "\"trap_redirect_flushes\":");
    let trap_redirect_flush_cycles = json_u64_field(&stdout, "\"trap_redirect_flush_cycles\":");
    let stage_trap_redirect_flushed =
        json_stage_summary(&stdout, "\"stage_trap_redirect_flushed\":{");
    let stage_trap_redirect_flushed_cycles =
        json_stage_summary(&stdout, "\"stage_trap_redirect_flushed_cycles\":{");
    let flush_cause_trap_redirect_flushed = ["fetch1", "fetch2", "decode", "execute", "commit"]
        .map(|stage| {
            stat_value(
                &stdout,
                &format!(
                    "sim.cpu0.pipeline.in_order.flush_cause.trap_redirect.stage.{stage}.flushed"
                ),
            )
        });
    let flush_cause_trap_redirect_flushed_cycles =
        ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
            stat_value(
                &stdout,
                &format!(
                "sim.cpu0.pipeline.in_order.flush_cause.trap_redirect.stage.{stage}.flushed_cycles"
            ),
            )
        });

    assert_eq!(
        json.pointer("/simulation/status").and_then(Value::as_str),
        Some("executed_until_trap")
    );
    assert_eq!(
        json.pointer("/simulation/trap").and_then(Value::as_str),
        Some("environment_call")
    );
    assert_eq!(
        json.pointer("/simulation/trap_pc").and_then(Value::as_str),
        Some("0x80000000")
    );
    assert_eq!(
        json.pointer("/cores/0/in_order_pipeline/trap_redirects")
            .and_then(Value::as_u64),
        Some(trap_redirects)
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_redirects"
        ),
        branch_prediction_redirects
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.trap_redirects"),
        trap_redirects
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.pipeline.in_order.trap_redirect_flushes"),
        trap_redirect_flushes
    );
    assert_eq!(
        stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.trap_redirect_flush_cycles"
        ),
        trap_redirect_flush_cycles
    );
    assert_eq!(branch_prediction_redirects, 0, "{stdout}");
    assert_eq!(trap_redirects, 1, "{stdout}");
    assert!(
        trap_redirect_flushes > 0,
        "trap redirect should flush younger wrong-path in-flight instructions\n{stdout}"
    );
    assert!(
        trap_redirect_flush_cycles > 0,
        "trap redirect should account a non-branch squash cycle\n{stdout}"
    );
    assert_eq!(
        stage_trap_redirect_flushed.iter().sum::<u64>(),
        trap_redirect_flushes
    );
    assert!(
        stage_trap_redirect_flushed.iter().any(|value| *value > 0),
        "{stdout}"
    );
    assert!(
        stage_trap_redirect_flushed_cycles
            .iter()
            .any(|value| *value > 0),
        "{stdout}"
    );
    assert_eq!(
        stage_trap_redirect_flushed_cycles.iter().sum::<u64>(),
        trap_redirect_flush_cycles
    );
    assert_eq!(
        flush_cause_trap_redirect_flushed,
        stage_trap_redirect_flushed
    );
    assert_eq!(
        flush_cause_trap_redirect_flushed_cycles,
        stage_trap_redirect_flushed_cycles
    );
    assert!(
        flush_cause_trap_redirect_flushed_cycles
            .iter()
            .any(|value| *value > 0),
        "{stdout}"
    );
    for (index, stage) in ["fetch1", "fetch2", "decode", "execute", "commit"]
        .iter()
        .enumerate()
    {
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.trap_redirect_flushed")
            ),
            stage_trap_redirect_flushed[index]
        );
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.trap_redirect_flushed_cycles")
            ),
            stage_trap_redirect_flushed_cycles[index]
        );
    }
    assert!(!stdout.contains("\"x6\":\"0x9\""));
}

#[test]
fn rem6_run_text_stats_emit_in_order_trap_redirect_aliases_from_execution() {
    let program = riscv64_program(&[
        0x0000_0073,                // ecall redirects to the trap vector
        0x0010_0313,                // wrong path after the trap
        i_type(7, 0, 0x0, 7, 0x13), // trap-vector target after default mtvec
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-trap-redirect-text-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "80",
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
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionRedirects"
        ),
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.branch_prediction_redirects"
        )
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.trapRedirects"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.trap_redirects")
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.trapRedirectFlushes"),
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.trap_redirect_flushes")
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.trapRedirectFlushCycles"
        ),
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.trap_redirect_flush_cycles"
        )
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.branchPredictionRedirects"
        ),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.trapRedirects"),
        1
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.pipeline.inOrder.trapRedirectFlushes"),
        0
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.pipeline.inOrder.trapRedirectFlushCycles"
        ),
        0
    );
    for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.trapRedirectFlushed")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.trap_redirect_flushed")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.trapRedirectFlushedCycles")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu0.pipeline.in_order.stage.{stage}.trap_redirect_flushed_cycles")
            )
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.trapRedirectFlushed")
            ),
            0
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu.pipeline.inOrder.stage.{stage}.trapRedirectFlushedCycles")
            ),
            0
        );
    }
    assert!(
        text_stat_line(&stdout, "system.cpu.pipeline.inOrder.trapRedirects").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(
            &stdout,
            "system.cpu.pipeline.inOrder.trapRedirectFlushCycles"
        )
        .contains("unit=Cycle"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_text_stats_emit_positive_btb_eviction_alias() {
    let mut words = vec![0x0000_0013; 162]; // addi x0, x0, 0
    for index in [0, 32, 64, 96, 128] {
        words[index] = j_type(128, 0);
    }
    words[160] = 0x0070_0293; // addi x5, x0, 7
    words[161] = 0x0000_0073; // ecall
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &riscv64_program(&words));
    let path = temp_binary("in-order-branch-prediction-btb-eviction-alias", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "900",
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
    let btb_evictions = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.evictions");

    assert!(btb_evictions > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.evictions"),
        btb_evictions
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.btb.misses::total"),
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.misses")
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.btb.evictions").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_stats_emit_ras_target_provider_from_real_call_return_fetch() {
    let program = riscv64_program(&[
        j_type(12, 1),              // jal x1, function
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        0x0000_0073,                // ecall
        i_type(1, 0, 0x0, 6, 0x13), // function: addi x6, x0, 1
        i_type(0, 1, 0x0, 0, 0x67), // jalr x0, 0(x1)
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-ras-provider", &elf);

    let run = |stats_format: &str| {
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
                stats_format,
                "--execute",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let stdout = run("json");
    let ras_provider = stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.ras");
    let ras_pushes = stat_value(&stdout, "sim.cpu0.branch_predictor.ras.pushes");
    let ras_pops = stat_value(&stdout, "sim.cpu0.branch_predictor.ras.pops");
    let ras_squashes = stat_value(&stdout, "sim.cpu0.branch_predictor.ras.squashes");
    let ras_used = stat_value(&stdout, "sim.cpu0.branch_predictor.ras.used");
    let ras_correct = stat_value(&stdout, "sim.cpu0.branch_predictor.ras.correct");
    let ras_incorrect = stat_value(&stdout, "sim.cpu0.branch_predictor.ras.incorrect");
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    assert!(ras_provider > 0, "{stdout}");
    assert!(ras_pushes > 0, "{stdout}");
    assert!(ras_pops > 0, "{stdout}");
    assert_eq!(ras_squashes, 0, "{stdout}");
    assert_eq!(ras_used, ras_correct + ras_incorrect, "{stdout}");
    assert_eq!(ras_correct, ras_used, "{stdout}");
    assert_eq!(ras_incorrect, 0, "{stdout}");
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/ras/pushes")
            .and_then(Value::as_u64),
        Some(ras_pushes)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/ras/pops")
            .and_then(Value::as_u64),
        Some(ras_pops)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/ras/used")
            .and_then(Value::as_u64),
        Some(ras_used)
    );
    for name in ["pushes", "pops", "squashes", "used", "correct", "incorrect"] {
        assert_json_stat_alias(
            &artifact,
            &format!("sim.cpu0.branch_predictor.ras.{name}"),
            &format!("system.cpu.branchPred.ras.{name}"),
        );
    }
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.total"),
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_provider.no_target"
        ) + stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.btb")
            + ras_provider
            + stat_value(
                &stdout,
                "sim.cpu0.branch_predictor.target_provider.indirect"
            )
    );
    assert!(
        stdout.contains("\"x5\":\"0x7\""),
        "return path did not execute after RAS-predicted jalr:\n{stdout}"
    );
    assert!(
        stdout.contains("\"x6\":\"0x1\""),
        "function body did not execute before RAS-predicted jalr:\n{stdout}"
    );

    let stdout = run("text");
    let ras_provider = text_stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.ras");
    assert!(ras_provider > 0, "{stdout}");
    for name in ["pushes", "pops", "squashes", "used", "correct", "incorrect"] {
        let raw = text_stat_value(&stdout, &format!("sim.cpu0.branch_predictor.ras.{name}"));
        assert_eq!(
            text_stat_value(&stdout, &format!("system.cpu.branchPred.ras.{name}")),
            raw
        );
        assert!(
            text_stat_line(&stdout, &format!("system.cpu.branchPred.ras.{name}"))
                .contains("unit=Count"),
            "{stdout}"
        );
    }
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::RAS"),
        ras_provider
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetProvider_0::total"),
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.target_provider.total")
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.targetProvider_0::RAS")
            .contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_stats_emit_multicore_ras_target_provider_from_real_call_return_fetch() {
    let program = riscv64_program(&[
        j_type(12, 1),              // jal x1, function
        i_type(7, 0, 0x0, 5, 0x13), // addi x5, x0, 7
        j_type(0, 0),               // done: jal x0, done
        i_type(1, 0, 0x0, 6, 0x13), // function: addi x6, x0, 1
        i_type(0, 1, 0x0, 0, 0x67), // jalr x0, 0(x1)
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-multicore-branch-ras-provider", &elf);

    let run = |stats_format: &str| {
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
                stats_format,
                "--execute",
                "--cores",
                "2",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let stdout = run("json");
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    for cpu in [0, 1] {
        let ras_provider = stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.target_provider.ras"),
        );
        let ras_pushes = stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.ras.pushes"),
        );
        let ras_pops = stat_value(&stdout, &format!("sim.cpu{cpu}.branch_predictor.ras.pops"));
        let ras_squashes = stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.ras.squashes"),
        );
        let ras_used = stat_value(&stdout, &format!("sim.cpu{cpu}.branch_predictor.ras.used"));
        let ras_correct = stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.ras.correct"),
        );
        let ras_incorrect = stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.ras.incorrect"),
        );
        assert!(ras_provider > 0, "{stdout}");
        assert!(ras_pushes > 0, "{stdout}");
        assert!(ras_pops > 0, "{stdout}");
        assert_eq!(ras_squashes, 0, "{stdout}");
        assert_eq!(ras_used, ras_correct + ras_incorrect, "{stdout}");
        assert_eq!(ras_correct, ras_used, "{stdout}");
        assert_eq!(ras_incorrect, 0, "{stdout}");
        assert_eq!(
            artifact
                .pointer(&format!("/cores/{cpu}/branch_predictor/ras/pushes"))
                .and_then(Value::as_u64),
            Some(ras_pushes)
        );
        assert_eq!(
            artifact
                .pointer(&format!("/cores/{cpu}/branch_predictor/ras/pops"))
                .and_then(Value::as_u64),
            Some(ras_pops)
        );
        assert_eq!(
            artifact
                .pointer(&format!("/cores/{cpu}/branch_predictor/ras/used"))
                .and_then(Value::as_u64),
            Some(ras_used)
        );
        for name in ["pushes", "pops", "squashes", "used", "correct", "incorrect"] {
            assert_json_stat_alias(
                &artifact,
                &format!("sim.cpu{cpu}.branch_predictor.ras.{name}"),
                &format!("system.cpu{cpu}.branchPred.ras.{name}"),
            );
        }
        assert_eq!(
            stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.total")
            ),
            stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.no_target")
            ) + stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.btb")
            ) + ras_provider
                + stat_value(
                    &stdout,
                    &format!("sim.cpu{cpu}.branch_predictor.target_provider.indirect")
                )
        );
    }
    for cpu in [0, 1] {
        assert_eq!(
            json_core_register(&artifact, cpu, "x5"),
            Some("0x7"),
            "core {cpu} did not execute the return path:\n{stdout}"
        );
        assert_eq!(
            json_core_register(&artifact, cpu, "x6"),
            Some("0x1"),
            "core {cpu} did not execute the function body:\n{stdout}"
        );
    }
    assert_json_stat_absent(&artifact, "system.cpu.branchPred.ras.pushes");

    let stdout = run("text");
    for cpu in [0, 1] {
        let ras_provider = text_stat_value(
            &stdout,
            &format!("sim.cpu{cpu}.branch_predictor.target_provider.ras"),
        );
        assert!(ras_provider > 0, "{stdout}");
        for name in ["pushes", "pops", "squashes", "used", "correct", "incorrect"] {
            let raw = text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.ras.{name}"),
            );
            assert_eq!(
                text_stat_value(&stdout, &format!("system.cpu{cpu}.branchPred.ras.{name}")),
                raw
            );
            assert!(
                text_stat_line(&stdout, &format!("system.cpu{cpu}.branchPred.ras.{name}"))
                    .contains("unit=Count"),
                "{stdout}"
            );
        }
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::RAS")
            ),
            ras_provider
        );
        assert_eq!(
            text_stat_value(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::total")
            ),
            text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.total")
            )
        );
        assert!(
            text_stat_line(
                &stdout,
                &format!("system.cpu{cpu}.branchPred.targetProvider_0::RAS")
            )
            .contains("unit=Count"),
            "{stdout}"
        );
    }
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.branchPred.targetProvider_0::RAS"
    ));
    assert!(!has_text_stat(&stdout, "system.cpu.branchPred.ras.pushes"));
}

#[test]
fn rem6_run_stats_emit_indirect_target_provider_from_real_jalr_fetch() {
    let program = riscv64_program(&[
        i_type(0, 10, 0x0, 0, 0x67), // jalr x0, 0(x10)
        i_type(1, 0, 0x0, 5, 0x13),  // skipped: addi x5, x0, 1
        i_type(9, 0, 0x0, 5, 0x13),  // target: addi x5, x0, 9
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-indirect-provider", &elf);

    for cores in [1, 2] {
        let run = |stats_format: &str| {
            let cores_arg = cores.to_string();
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
                    "--riscv-boot-a0",
                    "0x80000008",
                    "--stats-format",
                    stats_format,
                    "--execute",
                    "--cores",
                    &cores_arg,
                ])
                .output()
                .unwrap();

            assert!(
                output.status.success(),
                "stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            String::from_utf8(output.stdout).unwrap()
        };

        let stdout = run("json");
        let artifact: Value = serde_json::from_str(&stdout).unwrap();
        for cpu in 0..cores {
            let indirect_provider = stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.indirect"),
            );
            let indirect_hits = stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_hits"),
            );
            let indirect_mispredicted = stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_mispredicted"),
            );
            let expected_indirect_mispredicted = stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.indirect_conditional"),
            ) + stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.indirect_unconditional"),
            ) + stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.call_indirect"),
            );
            assert!(indirect_provider > 0, "{stdout}");
            assert_eq!(indirect_hits, indirect_provider);
            assert_eq!(indirect_mispredicted, expected_indirect_mispredicted);
            let indirect_lookups = stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.indirect_conditional"),
            ) + stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.indirect_unconditional"),
            ) + stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.call_indirect"),
            );
            let alias_prefix = if cores == 1 {
                "system.cpu".to_string()
            } else {
                format!("system.cpu{cpu}")
            };
            assert!(indirect_lookups > 0, "{stdout}");
            assert_json_stat_count_value(
                &artifact,
                &format!("{alias_prefix}.branchPred.indirectLookups"),
                indirect_lookups,
            );
            assert_json_stat_alias(
                &artifact,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_hits"),
                &format!("{alias_prefix}.branchPred.indirectHits"),
            );
            assert_json_stat_count_value(
                &artifact,
                &format!("{alias_prefix}.branchPred.indirectMisses"),
                indirect_lookups - indirect_hits,
            );
            assert_json_stat_alias(
                &artifact,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_mispredicted"),
                &format!("{alias_prefix}.branchPred.indirectMispredicted"),
            );
            assert_eq!(
                artifact
                    .pointer(&format!(
                        "/cores/{cpu}/branch_predictor/indirect_mispredicted"
                    ))
                    .and_then(Value::as_u64),
                Some(indirect_mispredicted)
            );
            assert_eq!(
                stat_value(
                    &stdout,
                    &format!("sim.cpu{cpu}.branch_predictor.target_provider.total")
                ),
                stat_value(
                    &stdout,
                    &format!("sim.cpu{cpu}.branch_predictor.target_provider.no_target")
                ) + stat_value(
                    &stdout,
                    &format!("sim.cpu{cpu}.branch_predictor.target_provider.btb")
                ) + stat_value(
                    &stdout,
                    &format!("sim.cpu{cpu}.branch_predictor.target_provider.ras")
                ) + indirect_provider
            );
        }
        if cores == 2 {
            assert_json_stat_absent(&artifact, "system.cpu.branchPred.indirectLookups");
            assert_json_stat_absent(&artifact, "system.cpu.branchPred.indirectHits");
            assert_json_stat_absent(&artifact, "system.cpu.branchPred.indirectMisses");
        }
        for cpu in 0..cores {
            assert_eq!(
                json_core_register(&artifact, cpu, "x5"),
                Some("0x9"),
                "core {cpu} did not execute the indirect jalr target path:\n{stdout}"
            );
        }

        let stdout = run("text");
        for cpu in 0..cores {
            let alias_prefix = if cores == 1 {
                "system.cpu".to_string()
            } else {
                format!("system.cpu{cpu}")
            };
            let indirect_provider = text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.target_provider.indirect"),
            );
            let indirect_hits = text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_hits"),
            );
            let indirect_mispredicted = text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_mispredicted"),
            );
            let expected_indirect_mispredicted = text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.indirect_conditional"),
            ) + text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.indirect_unconditional"),
            ) + text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.mispredicted.call_indirect"),
            );
            let indirect_lookups = text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.indirect_conditional"),
            ) + text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.indirect_unconditional"),
            ) + text_stat_value(
                &stdout,
                &format!("sim.cpu{cpu}.branch_predictor.lookups.call_indirect"),
            );
            assert!(indirect_provider > 0, "{stdout}");
            assert_eq!(indirect_hits, indirect_provider);
            assert_eq!(indirect_mispredicted, expected_indirect_mispredicted);
            assert!(indirect_lookups > 0, "{stdout}");
            assert_eq!(
                text_stat_value(
                    &stdout,
                    &format!("{alias_prefix}.branchPred.indirectLookups")
                ),
                indirect_lookups
            );
            assert_eq!(
                text_stat_value(&stdout, &format!("{alias_prefix}.branchPred.indirectHits")),
                indirect_hits
            );
            assert_eq!(
                text_stat_value(
                    &stdout,
                    &format!("{alias_prefix}.branchPred.indirectMisses")
                ),
                indirect_lookups - indirect_hits
            );
            assert_eq!(
                text_stat_value(
                    &stdout,
                    &format!("{alias_prefix}.branchPred.indirectMispredicted")
                ),
                indirect_mispredicted
            );
            assert_eq!(
                text_stat_value(
                    &stdout,
                    &format!("{alias_prefix}.branchPred.targetProvider_0::Indirect")
                ),
                indirect_provider
            );
            assert!(
                text_stat_line(
                    &stdout,
                    &format!("{alias_prefix}.branchPred.targetProvider_0::Indirect")
                )
                .contains("unit=Count"),
                "{stdout}"
            );
            assert!(
                text_stat_line(
                    &stdout,
                    &format!("{alias_prefix}.branchPred.indirectMispredicted")
                )
                .contains("unit=Count"),
                "{stdout}"
            );
        }
    }
}

#[test]
fn rem6_run_stats_emit_indirect_call_branch_kind_from_real_jalr_fetch() {
    let program = riscv64_program(&[
        i_type(0, 10, 0x0, 1, 0x67), // jalr x1, 0(x10)
        i_type(7, 0, 0x0, 5, 0x13),  // return: addi x5, x0, 7
        0x0000_0073,                 // ecall
        i_type(1, 0, 0x0, 6, 0x13),  // function: addi x6, x0, 1
        i_type(0, 1, 0x0, 0, 0x67),  // jalr x0, 0(x1)
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-indirect-call-kind", &elf);

    let run = |stats_format: &str| {
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
                "--riscv-boot-a0",
                "0x8000000c",
                "--stats-format",
                stats_format,
                "--execute",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let stdout = run("json");
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    let call_indirect_lookups =
        stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.call_indirect");
    let call_indirect_committed =
        stat_value(&stdout, "sim.cpu0.branch_predictor.committed.call_indirect");
    let call_indirect_mispredicted = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredicted.call_indirect",
    );
    let call_indirect_corrected =
        stat_value(&stdout, "sim.cpu0.branch_predictor.corrected.call_indirect");
    let call_indirect_target_wrong = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.target_wrong.call_indirect",
    );
    let call_indirect_btb_miss = stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.call_indirect",
    );
    assert!(call_indirect_lookups > 0, "{stdout}");
    assert!(call_indirect_committed > 0, "{stdout}");
    assert_eq!(call_indirect_mispredicted, 0, "{stdout}");
    assert_eq!(call_indirect_corrected, call_indirect_mispredicted);
    assert_eq!(call_indirect_target_wrong, call_indirect_mispredicted);
    assert_eq!(call_indirect_btb_miss, call_indirect_mispredicted);
    assert!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.btb.mispredictions") > 0,
        "{stdout}"
    );
    assert!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_provider.indirect"
        ) > 0,
        "{stdout}"
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.indirect_hits"),
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_provider.indirect"
        )
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.indirect_mispredicted"),
        call_indirect_mispredicted
    );
    assert!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.return") > 0,
        "{stdout}"
    );
    assert!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.committed.return") > 0,
        "{stdout}"
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/lookups/call_indirect")
            .and_then(Value::as_u64),
        Some(call_indirect_lookups)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/mispredicted/call_indirect")
            .and_then(Value::as_u64),
        Some(call_indirect_mispredicted)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/corrected/call_indirect")
            .and_then(Value::as_u64),
        Some(call_indirect_corrected)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/target_wrong/call_indirect")
            .and_then(Value::as_u64),
        Some(call_indirect_target_wrong)
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/btb/mispredict_due_to_btb_miss/call_indirect")
            .and_then(Value::as_u64),
        Some(call_indirect_btb_miss)
    );
    assert_eq!(json_core_register(&artifact, 0, "x5"), Some("0x7"));
    assert_eq!(json_core_register(&artifact, 0, "x6"), Some("0x1"));

    let stdout = run("text");
    let call_indirect_lookups =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.call_indirect");
    let call_indirect_committed =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.committed.call_indirect");
    let call_indirect_mispredicted = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.mispredicted.call_indirect",
    );
    let call_indirect_corrected =
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.corrected.call_indirect");
    let call_indirect_target_wrong = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.target_wrong.call_indirect",
    );
    let call_indirect_btb_miss = text_stat_value(
        &stdout,
        "sim.cpu0.branch_predictor.btb.mispredict_due_to_btb_miss.call_indirect",
    );
    assert!(call_indirect_lookups > 0, "{stdout}");
    assert!(call_indirect_committed > 0, "{stdout}");
    assert_eq!(call_indirect_mispredicted, 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.lookups_0::CallIndirect"),
        call_indirect_lookups
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.committed_0::CallIndirect"),
        call_indirect_committed
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredicted_0::CallIndirect"
        ),
        call_indirect_mispredicted
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.corrected_0::CallIndirect"),
        call_indirect_corrected
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.targetWrong_0::CallIndirect"),
        call_indirect_target_wrong
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "system.cpu.branchPred.mispredictDueToBTBMiss_0::CallIndirect"
        ),
        call_indirect_btb_miss
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.indirectMispredicted"),
        call_indirect_mispredicted
    );
    assert!(
        text_stat_value(&stdout, "system.cpu.branchPred.BTBMispredicted") > 0,
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.branchPred.lookups_0::CallIndirect")
            .contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_stats_exclude_return_jalr_from_indirect_hit_aliases() {
    let program = riscv64_program(&[
        i_type(0, 11, 0x0, 1, 0x13), // addi x1, x11, 0
        i_type(0, 1, 0x0, 0, 0x67),  // jalr x0, 0(x1)
        i_type(1, 0, 0x0, 5, 0x13),  // skipped: addi x5, x0, 1
        i_type(9, 0, 0x0, 5, 0x13),  // target: addi x5, x0, 9
        0x0000_0073,                 // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-branch-return-provider-not-indirect-hit", &elf);

    let run = |stats_format: &str| {
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
                "--riscv-boot-a1",
                "0x8000000c",
                "--stats-format",
                stats_format,
                "--execute",
            ])
            .output()
            .unwrap();

        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };

    let stdout = run("json");
    let artifact: Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        json_core_register(&artifact, 0, "x5"),
        Some("0x9"),
        "return-class jalr target path did not execute:\n{stdout}"
    );
    assert!(
        stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_provider.indirect"
        ) > 0,
        "{stdout}"
    );
    assert!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.return") > 0,
        "{stdout}"
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.indirect_hits"),
        0
    );
    assert_eq!(
        stat_value(&stdout, "sim.cpu0.branch_predictor.indirect_mispredicted"),
        0
    );
    assert_eq!(
        artifact
            .pointer("/cores/0/branch_predictor/indirect_mispredicted")
            .and_then(Value::as_u64),
        Some(0)
    );

    let stdout = run("text");
    assert!(
        text_stat_value(
            &stdout,
            "sim.cpu0.branch_predictor.target_provider.indirect"
        ) > 0,
        "{stdout}"
    );
    assert!(
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.lookups.return") > 0,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.indirectLookups"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.indirectHits"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.indirectMisses"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "sim.cpu0.branch_predictor.indirect_mispredicted"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.indirectMispredicted"),
        0
    );
}

#[test]
fn rem6_run_text_stats_do_not_count_unconditional_jumps_as_cond_branch_predictions() {
    let program = riscv64_program(&[
        0x0070_0293,  // addi x5, x0, 7
        j_type(8, 0), // jal x0, target
        0x0010_0313,  // addi x6, x0, 1
        0x0000_0073,  // ecall
    ]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-unconditional-branch-prediction-aliases", &elf);

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "28",
            "--memory-route-delay",
            "5",
            "--stats-format",
            "text",
            "--execute",
            "--memory-system",
            "direct",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        text_stat_value(&stdout, "sim.cpu0.pipeline.in_order.branch_predictions") > 0,
        "{stdout}"
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_predictions"
        ),
        0
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_predicted_taken"
        ),
        0
    );
    assert_eq!(
        text_stat_value(
            &stdout,
            "sim.cpu0.pipeline.in_order.conditional_branch_mispredictions"
        ),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.condPredicted"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.condPredictedTaken"),
        0
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.condIncorrect"),
        0
    );
    let btb_lookups = text_stat_value(&stdout, "sim.cpu0.branch_predictor.btb.lookups");
    assert!(btb_lookups > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.branchPred.BTBLookups"),
        btb_lookups
    );
}

fn o3_branch_prediction_alias_binary(name: &str) -> std::path::PathBuf {
    let data_start = 96_i32;
    let words = vec![
        i_type(1, 0, 0x0, 7, 0x13),
        i_type(1, 0, 0x0, 9, 0x13),
        b_type(12, 0, 9, 0x1),
        i_type(11, 0, 0x0, 6, 0x13),
        j_type(16, 0),
        m5op(M5_SWITCH_CPU),
        i_type(0, 0, 0x0, 9, 0x13),
        j_type(-20, 0),
        u_type(0, 10, 0x17),
        i_type(data_start - 32, 10, 0x0, 10, 0x13),
        s_type(0, 6, 10, 0b011),
        s_type(8, 9, 10, 0b011),
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ];
    let mut words = words;
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0, 0, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn o3_store_conditional_failure_binary(name: &str) -> std::path::PathBuf {
    let mut words = vec![m5op(M5_SWITCH_CPU)];
    let auipc_pc = (words.len() * 4) as i32;
    let data_start = 64_i32;
    words.extend([
        u_type(0, 5, 0x17),                             // auipc x5, 0
        i_type(data_start - auipc_pc, 5, 0x0, 5, 0x13), // addi x5, x5, data
        i_type(0x2a, 0, 0x0, 6, 0x13),                  // addi x6, x0, 0x2a
        atomic_type(0x03, false, false, 6, 5, 0x3, 7),  // sc.d x7, x6, (x5)
        s_type(8, 7, 5, 0b011),                         // sd x7, 8(x5)
        m5op(M5_EXIT),
        m5op(M5_FAIL),
    ]);
    while words.len() * 4 < data_start as usize {
        words.push(0);
    }
    words.extend([0x5566_7788, 0x1122_3344, 0, 0]);
    let program = riscv64_program(&words);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn m5op(function: u32) -> u32 {
    (function << 25) | 0x7b
}

fn json_u64_field(stdout: &str, marker: &str) -> u64 {
    let start = stdout
        .find(marker)
        .unwrap_or_else(|| panic!("missing JSON field {marker} in output:\n{stdout}"))
        + marker.len();
    let end = stdout[start..]
        .find(|character: char| !character.is_ascii_digit())
        .map(|offset| start + offset)
        .unwrap_or(stdout.len());
    stdout[start..end]
        .parse::<u64>()
        .unwrap_or_else(|error| panic!("invalid numeric JSON field {marker}: {error}"))
}

fn json_object_u64_field(stdout: &str, object_marker: &str, field_marker: &str) -> u64 {
    let object_start = stdout
        .find(object_marker)
        .unwrap_or_else(|| panic!("missing JSON object {object_marker} in output:\n{stdout}"))
        + object_marker.len();
    let object_end = stdout[object_start..]
        .find('}')
        .map(|offset| object_start + offset)
        .unwrap_or_else(|| panic!("unterminated JSON object {object_marker} in output:\n{stdout}"));
    json_u64_field(&stdout[object_start..object_end], field_marker)
}

fn json_stage_summary(stdout: &str, marker: &str) -> [u64; 5] {
    let start = stdout
        .find(marker)
        .unwrap_or_else(|| panic!("missing JSON stage summary {marker} in output:\n{stdout}"))
        + marker.len();
    let end = stdout[start..]
        .find('}')
        .map(|offset| start + offset)
        .unwrap_or_else(|| panic!("unterminated JSON stage summary {marker} in output:\n{stdout}"));
    let object = &stdout[start..end];
    [
        json_u64_field(object, "\"fetch1\":"),
        json_u64_field(object, "\"fetch2\":"),
        json_u64_field(object, "\"decode\":"),
        json_u64_field(object, "\"execute\":"),
        json_u64_field(object, "\"commit\":"),
    ]
}

fn json_core_register<'a>(artifact: &'a Value, cpu: i32, register: &str) -> Option<&'a str> {
    artifact
        .pointer(&format!("/cores/{cpu}/registers/{register}"))
        .and_then(Value::as_str)
}

fn assert_in_order_cause_stage_aliases(
    stdout: &str,
    source_family: &str,
    alias_family: &str,
    source_cause: &str,
    alias_cause: &str,
) -> ([u64; 5], [u64; 5]) {
    let records_alias = format!("system.cpu.pipeline.inOrder.{alias_family}.{alias_cause}.records");
    let records_source =
        format!("sim.cpu0.pipeline.in_order.{source_family}.{source_cause}.records");
    assert_eq!(
        text_stat_value(stdout, &records_alias),
        text_stat_value(stdout, &records_source)
    );
    assert!(
        text_stat_line(stdout, &records_alias).contains("unit=Count"),
        "{stdout}"
    );
    let flushed_values = ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        let flushed_alias = format!(
            "system.cpu.pipeline.inOrder.{alias_family}.{alias_cause}.stage.{stage}.flushed"
        );
        let flushed_cycles_alias = format!(
            "system.cpu.pipeline.inOrder.{alias_family}.{alias_cause}.stage.{stage}.flushedCycles"
        );
        let flushed_source = format!(
            "sim.cpu0.pipeline.in_order.{source_family}.{source_cause}.stage.{stage}.flushed"
        );
        let flushed_cycles_source = format!(
            "sim.cpu0.pipeline.in_order.{source_family}.{source_cause}.stage.{stage}.flushed_cycles"
        );
        assert_eq!(
            text_stat_value(stdout, &flushed_alias),
            text_stat_value(stdout, &flushed_source)
        );
        assert_eq!(
            text_stat_value(stdout, &flushed_cycles_alias),
            text_stat_value(stdout, &flushed_cycles_source)
        );
        assert!(
            text_stat_line(stdout, &flushed_alias).contains("unit=Count"),
            "{stdout}"
        );
        assert!(
            text_stat_line(stdout, &flushed_cycles_alias).contains("unit=Cycle"),
            "{stdout}"
        );
        text_stat_value(stdout, &flushed_alias)
    });
    let flushed_cycle_values = ["fetch1", "fetch2", "decode", "execute", "commit"].map(|stage| {
        text_stat_value(
            stdout,
            &format!(
                "system.cpu.pipeline.inOrder.{alias_family}.{alias_cause}.stage.{stage}.flushedCycles"
            ),
        )
    });
    (flushed_values, flushed_cycle_values)
}

fn text_stat_decimal(stdout: &str, path: &str) -> String {
    stdout
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next()? != path {
                return None;
            }
            Some(fields.next()?.to_string())
        })
        .unwrap_or_else(|| panic!("missing text stat {path} in output:\n{stdout}"))
}

fn text_stat_line<'a>(stdout: &'a str, path: &str) -> &'a str {
    stdout
        .lines()
        .find(|line| line.split_whitespace().next() == Some(path))
        .unwrap_or_else(|| panic!("missing text stat {path} in output:\n{stdout}"))
}

fn text_stat_tag<'a>(line: &'a str, prefix: &str) -> &'a str {
    line.split_whitespace()
        .find_map(|field| field.strip_prefix(prefix))
        .unwrap_or_else(|| panic!("missing text stat tag {prefix} in line {line}"))
}

fn fixed_ratio(numerator: u64, denominator: u64) -> String {
    assert_ne!(denominator, 0);
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn fixed_ratio_precision(numerator: u64, denominator: u64, precision: usize) -> String {
    assert_ne!(denominator, 0);
    format!(
        "{:.precision$}",
        numerator as f64 / denominator as f64,
        precision = precision
    )
}

fn fixed_ratio_default_precision(numerator: u64, denominator: u64) -> String {
    assert_ne!(denominator, 0);
    let value = numerator as f64 / denominator as f64;
    if value == value.round() {
        format!("{value:.0}")
    } else {
        format!("{value:.6}")
    }
}

fn gem5_l1_cache_alias_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 32;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(0, 2, 0x3, 7, 0x03),                  // ld x7, 0(x2)
        i_type(1, 5, 0x0, 6, 0x13),                  // addi x6, x5, 1
        s_type(8, 6, 2, 0x3),                        // sd x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET, 0);
    program.extend_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program.extend_from_slice(&0u64.to_le_bytes());
    program.extend_from_slice(&[0; 16]);
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn text_stat_value(stdout: &str, path: &str) -> u64 {
    stdout
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            if fields.next()? != path {
                return None;
            }
            fields.next()?.parse().ok()
        })
        .unwrap_or_else(|| panic!("missing text stat {path} in output:\n{stdout}"))
}

fn text_stat_values_with_prefix(stdout: &str, prefix: &str) -> Vec<u64> {
    text_stat_lines_with_prefix(stdout, prefix)
        .iter()
        .map(|line| {
            line.split_whitespace()
                .nth(1)
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or_else(|| panic!("invalid text stat value in line {line}"))
        })
        .collect()
}

fn text_stat_lines_with_prefix<'a>(stdout: &'a str, prefix: &str) -> Vec<&'a str> {
    stdout
        .lines()
        .filter(|line| {
            line.split_whitespace()
                .next()
                .is_some_and(|path| path_has_numeric_suffix(path, prefix))
        })
        .collect()
}

fn json_stat_values_with_prefix(
    stdout: &str,
    prefix: &str,
    unit: &str,
    reset_policy: &str,
) -> Vec<u64> {
    stdout
        .split("{\"id\":")
        .skip(1)
        .filter_map(|tail| {
            let sample_end = tail.find('}').unwrap_or(tail.len());
            let sample = &tail[..sample_end];
            let path_tail = sample.split("\"path\":\"").nth(1)?;
            let path_end = path_tail.find('"')?;
            let path = &path_tail[..path_end];
            if !path_has_numeric_suffix(path, prefix) {
                return None;
            }
            assert!(
                sample.contains(&format!("\"unit\":\"{unit}\"")),
                "missing stat unit {unit} in {sample}"
            );
            assert!(
                sample.contains(&format!("\"reset_policy\":\"{reset_policy}\"")),
                "missing stat reset policy {reset_policy} in {sample}"
            );
            let value_tail = sample
                .split("\"value\":")
                .nth(1)
                .unwrap_or_else(|| panic!("missing stat value in {sample}"));
            let value_end = value_tail
                .find(',')
                .or_else(|| value_tail.find('}'))
                .unwrap_or(value_tail.len());
            Some(
                value_tail[..value_end]
                    .parse::<u64>()
                    .unwrap_or_else(|error| panic!("invalid stat value in {sample}: {error}")),
            )
        })
        .collect()
}

fn json_stat_record<'a>(json: &'a Value, path: &str) -> Option<&'a Value> {
    json.as_array()
        .or_else(|| json.pointer("/stats").and_then(Value::as_array))
        .and_then(|stats| {
            stats
                .iter()
                .find(|sample| sample.get("path").and_then(Value::as_str) == Some(path))
        })
}

fn json_stat_value(json: &Value, path: &str) -> u64 {
    json_stat_record(json, path)
        .and_then(|sample| sample.get("value").and_then(Value::as_u64))
        .unwrap_or_else(|| panic!("missing JSON stat value {path}: {json}"))
}

fn assert_json_stat_alias(json: &Value, source_path: &str, alias_path: &str) {
    let source = json_stat_record(json, source_path)
        .unwrap_or_else(|| panic!("missing JSON source stat {source_path}"));
    let alias = json_stat_record(json, alias_path)
        .unwrap_or_else(|| panic!("missing JSON alias stat {alias_path}"));

    assert_eq!(alias.get("value"), source.get("value"), "{alias_path}");
    assert_eq!(alias.get("unit"), source.get("unit"), "{alias_path}");
    assert_eq!(
        alias.get("reset_policy"),
        source.get("reset_policy"),
        "{alias_path}"
    );
}

fn assert_json_stat_count_value(json: &Value, path: &str, expected_value: u64) {
    let stat = json_stat_record(json, path).unwrap_or_else(|| panic!("missing JSON stat {path}"));

    assert_eq!(
        stat.get("value").and_then(Value::as_u64),
        Some(expected_value),
        "{path}"
    );
    assert_eq!(
        stat.get("unit").and_then(Value::as_str),
        Some("Count"),
        "{path}"
    );
}

fn assert_json_stat_absent(json: &Value, path: &str) {
    assert!(
        json_stat_record(json, path).is_none(),
        "unexpected JSON stat {path}"
    );
}

fn path_has_numeric_suffix(path: &str, prefix: &str) -> bool {
    path.strip_prefix(prefix).is_some_and(|suffix| {
        !suffix.is_empty() && suffix.chars().all(|character| character.is_ascii_digit())
    })
}

fn has_text_stat(stdout: &str, path: &str) -> bool {
    stdout
        .lines()
        .any(|line| line.split_whitespace().next() == Some(path))
}
