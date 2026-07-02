use std::{env, process::Command};

use crate::support::*;
use serde_json::Value;

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
fn rem6_run_text_stats_omit_ambiguous_gem5_l1_cache_aliases_for_multicore() {
    let path = gem5_l1_cache_alias_binary("gem5-l1-cache-aliases-multicore");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "220",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "2",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(text_stat_value(&stdout, "sim.instruction_cache.bank.accepted") > 0);
    assert!(text_stat_value(&stdout, "sim.data_cache.bank.accepted") > 0);
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.icache.overallMissRate"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.overallMissRate"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_cache_hit_miss_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l1-cache-aliases");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let icache_hits = text_stat_value(&stdout, "sim.instruction_cache.bank.immediate_hits");
    let icache_misses = text_stat_value(&stdout, "sim.instruction_cache.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.instruction_cache.bank.coalesced_misses");
    assert!(icache_hits > 0, "{stdout}");
    assert!(icache_misses > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallHits"),
        icache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallMisses"),
        icache_misses
    );
    let icache_accesses = icache_hits + icache_misses;
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandHits"),
        icache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandMisses"),
        icache_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandAccesses"),
        icache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.demandMissRate"),
        fixed_ratio(icache_misses, icache_accesses)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallAccesses"),
        icache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.overallMissRate"),
        fixed_ratio(icache_misses, icache_accesses)
    );
    let icache_mshr_hits = text_stat_value(&stdout, "sim.instruction_cache.bank.coalesced_misses");
    let icache_mshr_misses =
        text_stat_value(&stdout, "sim.instruction_cache.bank.scheduled_misses");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandMshrHits"),
        icache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallMshrHits"),
        icache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.demandMshrMisses"),
        icache_mshr_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.overallMshrMisses"),
        icache_mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.demandMshrMissRate"),
        fixed_ratio(icache_mshr_misses, icache_accesses)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.icache.overallMshrMissRate"),
        fixed_ratio(icache_mshr_misses, icache_accesses)
    );

    let dcache_hits = text_stat_value(&stdout, "sim.data_cache.bank.immediate_hits");
    let dcache_misses = text_stat_value(&stdout, "sim.data_cache.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.bank.coalesced_misses");
    assert!(dcache_hits > 0, "{stdout}");
    assert!(dcache_misses > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallHits"),
        dcache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallMisses"),
        dcache_misses
    );
    let dcache_accesses = dcache_hits + dcache_misses;
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandHits"),
        dcache_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandMisses"),
        dcache_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandAccesses"),
        dcache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.demandMissRate"),
        fixed_ratio(dcache_misses, dcache_accesses)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallAccesses"),
        dcache_accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.overallMissRate"),
        fixed_ratio(dcache_misses, dcache_accesses)
    );
    let dcache_mshr_hits = text_stat_value(&stdout, "sim.data_cache.bank.coalesced_misses");
    let dcache_mshr_misses = text_stat_value(&stdout, "sim.data_cache.bank.scheduled_misses");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandMshrHits"),
        dcache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallMshrHits"),
        dcache_mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.demandMshrMisses"),
        dcache_mshr_misses
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.overallMshrMisses"),
        dcache_mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.demandMshrMissRate"),
        fixed_ratio(dcache_mshr_misses, dcache_accesses)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.overallMshrMissRate"),
        fixed_ratio(dcache_mshr_misses, dcache_accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.demandMshrHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.demandMshrMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_l1_cache_hit_miss_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l1-cache-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.bank.immediate_hits",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.bank.immediate_hits",
        "Count",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.overallMshrMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.demandMshrMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.overallMshrMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.demandMshrMissRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l2_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l2-cache-overall-aliases");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let hits = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.immediate_hits")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.immediate_hits");
    let misses = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.instruction_cache.l2.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.coalesced_misses");
    let mshr_hits = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.coalesced_misses");
    let mshr_misses = text_stat_value(&stdout, "sim.instruction_cache.l2.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l2.bank.scheduled_misses");
    let accesses = hits + misses;
    assert!(misses > 0, "{stdout}");
    assert_eq!(text_stat_value(&stdout, "system.l2.overallHits"), hits);
    assert_eq!(text_stat_value(&stdout, "system.l2.overallMisses"), misses);
    assert_eq!(
        text_stat_value(&stdout, "system.l2.overallAccesses"),
        accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l2.overallMissRate"),
        fixed_ratio(misses, accesses)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.l2.overallMshrHits"),
        mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.l2.overallMshrMisses"),
        mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l2.overallMshrMissRate"),
        fixed_ratio(mshr_misses, accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMshrHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l2.overallMshrMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_l2_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l2-cache-overall-aliases-json");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.l2.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.l2.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.l2.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l2.overallMshrMissRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l3_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l3-cache-overall-aliases");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let hits = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.immediate_hits")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.immediate_hits");
    let misses = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.instruction_cache.l3.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.coalesced_misses");
    let mshr_hits = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.coalesced_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.coalesced_misses");
    let mshr_misses = text_stat_value(&stdout, "sim.instruction_cache.l3.bank.scheduled_misses")
        + text_stat_value(&stdout, "sim.data_cache.l3.bank.scheduled_misses");
    let accesses = hits + misses;
    assert!(misses > 0, "{stdout}");
    assert_eq!(text_stat_value(&stdout, "system.l3.overallHits"), hits);
    assert_eq!(text_stat_value(&stdout, "system.l3.overallMisses"), misses);
    assert_eq!(
        text_stat_value(&stdout, "system.l3.overallAccesses"),
        accesses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l3.overallMissRate"),
        fixed_ratio(misses, accesses)
    );
    assert_eq!(
        text_stat_value(&stdout, "system.l3.overallMshrHits"),
        mshr_hits
    );
    assert_eq!(
        text_stat_value(&stdout, "system.l3.overallMshrMisses"),
        mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.l3.overallMshrMissRate"),
        fixed_ratio(mshr_misses, accesses)
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMshrHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.l3.overallMshrMissRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_l3_cache_overall_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-l3-cache-overall-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-l2-protocol",
            "msi",
            "--instruction-cache-l3-protocol",
            "msi",
            "--data-cache-protocol",
            "msi",
            "--data-cache-l2-protocol",
            "msi",
            "--data-cache-l3-protocol",
            "msi",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.l3.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.l3.bank.scheduled_misses",
        "Count",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.l3.overallHits\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallAccesses\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMissRate\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMshrHits\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMshrMisses\""));
    assert!(!stdout.contains("\"path\":\"system.l3.overallMshrMissRate\""));
}

#[test]
fn rem6_run_text_stats_omit_gem5_l1_demand_aliases_when_prefetch_issued() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-demand-alias-prefetch-omission");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert!(text_stat_value(&stdout, "sim.data_cache.prefetch.issued") > 0);
    assert!(has_text_stat(&stdout, "system.cpu.dcache.overallHits"));
    assert!(has_text_stat(&stdout, "system.cpu.dcache.overallMshrHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandHits"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMisses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandAccesses"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMissRate"));
    assert!(!has_text_stat(&stdout, "system.cpu.dcache.demandMshrHits"));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.dcache.demandMshrMisses"
    ));
    assert!(!has_text_stat(
        &stdout,
        "system.cpu.dcache.demandMshrMissRate"
    ));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_issued_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-issued-aliases");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let icache_issued = text_stat_value(&stdout, "sim.instruction_cache.prefetch.issued");
    let dcache_issued = text_stat_value(&stdout, "sim.data_cache.prefetch.issued");
    assert!(icache_issued > 0, "{stdout}");
    assert!(dcache_issued > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfIssued"),
        icache_issued
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIssued"),
        dcache_issued
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfIssued").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfIssued").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_issued_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-issued-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.issued",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.issued",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfIssued"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.issued")
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIssued"),
        stat_value(&stdout, "sim.data_cache.prefetch.issued")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_identified_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-identified-aliases");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let icache_identified = text_stat_value(&stdout, "sim.instruction_cache.prefetch.identified");
    let dcache_identified = text_stat_value(&stdout, "sim.data_cache.prefetch.identified");
    assert!(icache_identified > 0, "{stdout}");
    assert!(dcache_identified > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfIdentified"),
        icache_identified
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIdentified"),
        dcache_identified
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfIdentified").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfIdentified").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_identified_aliases() {
    let path = tagged_next_line_prefetch_binary("gem5-l1-prefetcher-pf-identified-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.identified",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.identified",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfIdentified"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.identified")
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfIdentified"),
        stat_value(&stdout, "sim.data_cache.prefetch.identified")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_span_page_aliases() {
    let path = page_crossing_prefetch_binary("gem5-l1-prefetcher-pf-span-page-aliases");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let icache_span_page = text_stat_value(&stdout, "sim.instruction_cache.prefetch.span_page");
    let dcache_span_page = text_stat_value(&stdout, "sim.data_cache.prefetch.span_page");
    assert!(icache_span_page > 0, "{stdout}");
    assert!(dcache_span_page > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfSpanPage"),
        icache_span_page
    );
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfSpanPage"),
        dcache_span_page
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfSpanPage").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfSpanPage").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_span_page_aliases() {
    let path = page_crossing_prefetch_binary("gem5-l1-prefetcher-pf-span-page-aliases-json");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.span_page",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.span_page",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfSpanPage"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.span_page")
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfSpanPage"),
        stat_value(&stdout, "sim.data_cache.prefetch.span_page")
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_useful_span_page_alias() {
    let path = useful_span_page_prefetch_binary("gem5-l1-prefetcher-pf-useful-span-page-alias");

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
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
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUsefulSpanPage"),
        text_stat_value(&stdout, "sim.data_cache.prefetch.useful_span_page")
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfUsefulSpanPage")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfUsefulSpanPage"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_in_cache_alias() {
    let path = same_line_data_prefetch_binary("gem5-l1-prefetcher-pf-in-cache-alias");

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
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let dcache_in_cache = text_stat_value(&stdout, "sim.data_cache.prefetch.in_cache");
    assert!(dcache_in_cache > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfInCache"),
        dcache_in_cache
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfInCache").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfInCache"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_in_cache_alias() {
    let path = same_line_data_prefetch_binary("gem5-l1-prefetcher-pf-in-cache-alias-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.in_cache",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfInCache"),
        stat_value(&stdout, "sim.data_cache.prefetch.in_cache")
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.prefetcher.pfInCache\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_useful_alias() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-pf-useful-alias");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let dcache_useful = text_stat_value(&stdout, "sim.data_cache.prefetch.useful");
    assert!(dcache_useful > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUseful"),
        dcache_useful
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfUseful").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfUseful"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_prefetcher_pf_useful_alias() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-pf-useful-alias-json");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.data_cache.prefetch.useful",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUseful"),
        stat_value(&stdout, "sim.data_cache.prefetch.useful")
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.icache.prefetcher.pfUseful\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_pf_useful_but_miss_alias() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-pf-useful-but-miss-alias");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let dcache_useful_but_miss =
        text_stat_value(&stdout, "sim.data_cache.prefetch.useful_but_miss");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.pfUsefulButMiss"),
        dcache_useful_but_miss
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.pfUsefulButMiss")
            .contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.pfUsefulButMiss"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_late_and_unused_aliases() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-late-unused-aliases");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    for (alias, source) in [
        ("pfUnused", "unused"),
        ("pfHitInCache", "hit_in_cache"),
        ("pfHitInMSHR", "hit_in_mshr"),
        ("pfHitInWB", "hit_in_write_buffer"),
        ("pfLate", "late"),
    ] {
        let source_path = format!("sim.data_cache.prefetch.{source}");
        let alias_path = format!("system.cpu.dcache.prefetcher.{alias}");
        assert_eq!(
            text_stat_value(&stdout, &alias_path),
            text_stat_value(&stdout, &source_path),
            "{stdout}"
        );
        assert!(
            text_stat_line(&stdout, &alias_path).contains("unit=Count"),
            "{stdout}"
        );
        assert!(!stdout.contains(&format!("system.cpu.icache.prefetcher.{alias}")));
    }
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_prefetcher_accuracy_and_coverage_aliases() {
    let path = useful_data_prefetch_binary("gem5-l1-prefetcher-accuracy-coverage-aliases");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "200",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "1",
            "--data-cache-protocol",
            "msi",
            "--data-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let dcache_useful = text_stat_value(&stdout, "sim.data_cache.prefetch.useful");
    let dcache_issued = text_stat_value(&stdout, "sim.data_cache.prefetch.issued");
    let dcache_demand_mshr_misses =
        text_stat_value(&stdout, "sim.data_cache.prefetch.demand_mshr_misses");
    assert_eq!(dcache_useful, 1, "{stdout}");
    assert_eq!(dcache_issued, 2, "{stdout}");
    assert_eq!(dcache_demand_mshr_misses, 1, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.dcache.prefetcher.demandMshrMisses"),
        dcache_demand_mshr_misses
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.prefetcher.accuracy"),
        fixed_ratio(dcache_useful, dcache_issued)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.cpu.dcache.prefetcher.coverage"),
        fixed_ratio(
            dcache_useful,
            dcache_useful.saturating_add(dcache_demand_mshr_misses)
        )
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.accuracy").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.dcache.prefetcher.coverage").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.icache.prefetcher.accuracy"));
    assert!(!stdout.contains("system.cpu.icache.prefetcher.coverage"));
}

#[test]
fn rem6_run_text_stats_emit_gem5_l1_icache_prefetcher_pf_useful_alias() {
    let path = useful_instruction_prefetch_binary("gem5-l1-icache-prefetcher-pf-useful-alias");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let icache_useful = text_stat_value(&stdout, "sim.instruction_cache.prefetch.useful");
    assert!(icache_useful > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.cpu.icache.prefetcher.pfUseful"),
        icache_useful
    );
    assert!(
        text_stat_line(&stdout, "system.cpu.icache.prefetcher.pfUseful").contains("unit=Count"),
        "{stdout}"
    );
    assert!(!stdout.contains("system.cpu.dcache.prefetcher.pfUseful"));
}

#[test]
fn rem6_run_json_stats_emit_gem5_l1_icache_prefetcher_pf_useful_alias() {
    let path = useful_instruction_prefetch_binary("gem5-l1-icache-prefetcher-pf-useful-alias-json");

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
            "--cores",
            "1",
            "--instruction-cache-protocol",
            "msi",
            "--instruction-cache-prefetcher",
            "tagged-next-line",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "sim.instruction_cache.prefetch.useful",
        "Count",
        0,
        "monotonic",
    );
    assert_eq!(
        stat_value(&stdout, "system.cpu.icache.prefetcher.pfUseful"),
        stat_value(&stdout, "sim.instruction_cache.prefetch.useful")
    );
    assert!(!stdout.contains("\"path\":\"system.cpu.dcache.prefetcher.pfUseful\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_mem_ctrl_bandwidth_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-mem-ctrl-bandwidth-aliases");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let sim_freq = text_stat_value(&stdout, "simFreq");
    let final_tick = text_stat_value(&stdout, "finalTick");
    let read_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesReadSys");
    let written_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesWrittenSys");
    let dram_read_bytes = text_stat_value(&stdout, "system.mem_ctrl.dram.dramBytesRead");
    let dram_written_bytes = text_stat_value(&stdout, "system.mem_ctrl.dram.dramBytesWritten");
    assert!(final_tick > 0, "{stdout}");
    assert!(read_bytes > 0, "{stdout}");
    assert!(written_bytes > 0, "{stdout}");
    assert_eq!(dram_read_bytes, read_bytes);
    assert_eq!(dram_written_bytes, written_bytes);

    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.avgRdBWSys"),
        fixed_ratio_precision(read_bytes * sim_freq, final_tick, 8)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.avgWrBWSys"),
        fixed_ratio_precision(written_bytes * sim_freq, final_tick, 8)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.avgRdBWSys").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.avgWrBWSys").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.avgRdBW"),
        fixed_ratio_default_precision(dram_read_bytes * sim_freq, final_tick * 1_000_000)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.avgWrBW"),
        fixed_ratio_default_precision(dram_written_bytes * sim_freq, final_tick * 1_000_000)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.avgRdBW").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.avgWrBW").contains("unit=(Byte/Second)"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_mem_ctrl_bandwidth_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-mem-ctrl-bandwidth-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.bytesReadSys",
        "Byte",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.bytesWrittenSys",
        "Byte",
        0,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.avgRdBWSys\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.avgWrBWSys\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.avgRdBW\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.avgWrBW\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_nvm_interface_byte_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-nvm-interface-byte-aliases");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "600",
            "--stats-format",
            "text",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "nvm",
            "--dump-memory",
            "0x80000028:8",
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
        text_stat_value(&stdout, "sim.memory.dram.profile.technology.nvm"),
        1
    );
    let read_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesReadSys");
    let written_bytes = text_stat_value(&stdout, "system.mem_ctrl.bytesWrittenSys");
    assert!(read_bytes > 0, "{stdout}");
    assert!(written_bytes > 0, "{stdout}");
    assert_eq!(
        text_stat_value(&stdout, "system.mem_ctrl.dram.nvmBytesRead"),
        read_bytes
    );
    assert_eq!(
        text_stat_value(&stdout, "system.mem_ctrl.dram.nvmBytesWritten"),
        written_bytes
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.nvmBytesRead").contains("unit=Byte"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.nvmBytesWritten").contains("unit=Byte"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_nvm_interface_byte_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-nvm-interface-byte-aliases-json");

    let output = Command::new(env!("CARGO_BIN_EXE_rem6"))
        .args([
            "run",
            "--isa",
            "riscv",
            "--binary",
            path.to_str().unwrap(),
            "--max-tick",
            "600",
            "--stats-format",
            "json",
            "--execute",
            "--cores",
            "1",
            "--dram-memory",
            "--dram-memory-profile",
            "nvm",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat(
        &stdout,
        "sim.memory.dram.profile.technology.nvm",
        "Count",
        1,
        "constant",
    );
    let read_bytes = stat_value(&stdout, "system.mem_ctrl.bytesReadSys");
    let written_bytes = stat_value(&stdout, "system.mem_ctrl.bytesWrittenSys");
    assert!(read_bytes > 0, "{stdout}");
    assert!(written_bytes > 0, "{stdout}");
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.nvmBytesRead",
        "Byte",
        read_bytes,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.nvmBytesWritten",
        "Byte",
        written_bytes,
        "monotonic",
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-burst-aliases");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.writeBursts");
    let dram_read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let dram_write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    assert!(read_bursts > 0, "{stdout}");
    assert!(write_bursts > 0, "{stdout}");
    assert_eq!(dram_read_bursts, read_bursts);
    assert_eq!(dram_write_bursts, write_bursts);
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.readBursts").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.writeBursts").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-burst-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.dram.readBursts",
        "Count",
        0,
        "monotonic",
    );
    assert_stat_greater_than(
        &stdout,
        "system.mem_ctrl.dram.writeBursts",
        "Count",
        0,
        "monotonic",
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_per_bank_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-per-bank-burst-aliases");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    let per_bank_reads =
        text_stat_values_with_prefix(&stdout, "system.mem_ctrl.dram.perBankRdBursts.bank");
    let per_bank_writes =
        text_stat_values_with_prefix(&stdout, "system.mem_ctrl.dram.perBankWrBursts.bank");
    assert!(!per_bank_reads.is_empty(), "{stdout}");
    assert!(!per_bank_writes.is_empty(), "{stdout}");
    assert_eq!(per_bank_reads.iter().sum::<u64>(), read_bursts);
    assert_eq!(per_bank_writes.iter().sum::<u64>(), write_bursts);
    assert!(
        text_stat_lines_with_prefix(&stdout, "system.mem_ctrl.dram.perBankRdBursts.bank",)
            .iter()
            .all(|line| line.contains("unit=Count")),
        "{stdout}"
    );
    assert!(
        text_stat_lines_with_prefix(&stdout, "system.mem_ctrl.dram.perBankWrBursts.bank",)
            .iter()
            .all(|line| line.contains("unit=Count")),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_per_bank_burst_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-per-bank-burst-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let read_bursts = stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    let per_bank_reads = json_stat_values_with_prefix(
        &stdout,
        "system.mem_ctrl.dram.perBankRdBursts.bank",
        "Count",
        "monotonic",
    );
    let per_bank_writes = json_stat_values_with_prefix(
        &stdout,
        "system.mem_ctrl.dram.perBankWrBursts.bank",
        "Count",
        "monotonic",
    );
    assert!(!per_bank_reads.is_empty(), "{stdout}");
    assert!(!per_bank_writes.is_empty(), "{stdout}");
    assert_eq!(per_bank_reads.iter().sum::<u64>(), read_bursts);
    assert_eq!(per_bank_writes.iter().sum::<u64>(), write_bursts);
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_mem_acc_latency_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-mem-acc-latency-aliases");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let all_access_latency = text_stat_value(&stdout, "sim.memory.dram.total_ready_latency_ticks");
    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let read_latency = text_stat_value(&stdout, "system.mem_ctrl.dram.totMemAccLat");
    assert!(all_access_latency > 0, "{stdout}");
    assert!(read_latency > 0, "{stdout}");
    assert!(read_latency < all_access_latency, "{stdout}");
    assert!(read_bursts > 0, "{stdout}");
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.avgMemAccLat"),
        fixed_ratio_precision(read_latency, read_bursts, 2)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.totMemAccLat").contains("unit=Tick"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.avgMemAccLat").contains("unit=(Tick/Count)"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_mem_acc_latency_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-mem-acc-latency-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let all_access_latency = stat_value(&stdout, "sim.memory.dram.total_ready_latency_ticks");
    let read_latency = stat_value(&stdout, "system.mem_ctrl.dram.totMemAccLat");
    assert!(all_access_latency > 0, "{stdout}");
    assert!(read_latency > 0, "{stdout}");
    assert!(read_latency < all_access_latency, "{stdout}");
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.totMemAccLat",
        "Tick",
        read_latency,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.avgMemAccLat\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_row_hit_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-aliases");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let row_hits = text_stat_value(&stdout, "sim.memory.dram.row_hits");
    let read_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    assert_eq!(read_row_hits + write_row_hits, row_hits);
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.readRowHits").contains("unit=Count"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.writeRowHits").contains("unit=Count"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_emit_gem5_dram_interface_row_hit_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let row_hits = stat_value(&stdout, "sim.memory.dram.row_hits");
    let read_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    assert_eq!(read_row_hits + write_row_hits, row_hits);
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.readRowHits",
        "Count",
        read_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.writeRowHits",
        "Count",
        write_row_hits,
        "monotonic",
    );
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_row_hit_rate_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-rate-aliases");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let read_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = text_stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    assert!(read_bursts > 0, "{stdout}");
    assert!(write_bursts > 0, "{stdout}");
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.readRowHitRate"),
        fixed_ratio_precision(read_row_hits * 100, read_bursts, 2)
    );
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.writeRowHitRate"),
        fixed_ratio_precision(write_row_hits * 100, write_bursts, 2)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.readRowHitRate").contains("unit=Ratio"),
        "{stdout}"
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.writeRowHitRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_dram_interface_row_hit_rate_aliases() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-row-hit-rate-aliases-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let read_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.readRowHits");
    let write_row_hits = stat_value(&stdout, "system.mem_ctrl.dram.writeRowHits");
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.readRowHits",
        "Count",
        read_row_hits,
        "monotonic",
    );
    assert_stat(
        &stdout,
        "system.mem_ctrl.dram.writeRowHits",
        "Count",
        write_row_hits,
        "monotonic",
    );
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.readRowHitRate\""));
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.writeRowHitRate\""));
}

#[test]
fn rem6_run_text_stats_emit_gem5_dram_interface_page_hit_rate_alias() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-page-hit-rate-alias");

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
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();

    let row_hits = text_stat_value(&stdout, "sim.memory.dram.row_hits");
    let read_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.readBursts");
    let write_bursts = text_stat_value(&stdout, "system.mem_ctrl.dram.writeBursts");
    let total_bursts = read_bursts + write_bursts;
    assert!(total_bursts > 0, "{stdout}");
    assert_eq!(
        text_stat_decimal(&stdout, "system.mem_ctrl.dram.pageHitRate"),
        fixed_ratio_precision(row_hits * 100, total_bursts, 2)
    );
    assert!(
        text_stat_line(&stdout, "system.mem_ctrl.dram.pageHitRate").contains("unit=Ratio"),
        "{stdout}"
    );
}

#[test]
fn rem6_run_json_stats_omit_text_only_gem5_dram_interface_page_hit_rate_alias() {
    let path = gem5_l1_cache_alias_binary("gem5-dram-interface-page-hit-rate-alias-json");

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
            "json",
            "--execute",
            "--cores",
            "1",
            "--dump-memory",
            "0x80000028:8",
        ])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_stat_greater_than(&stdout, "sim.memory.dram.accesses", "Count", 0, "monotonic");
    assert!(!stdout.contains("\"path\":\"system.mem_ctrl.dram.pageHitRate\""));
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

    let width_one = in_order_pipeline_stats_for_width(&path, 1);
    let width_two = in_order_pipeline_stats_for_width(&path, 2);

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
    }
    assert_stat_greater_than(
        &width_two,
        "sim.cpu0.pipeline.in_order.cycles",
        "Cycle",
        stat_value(&width_two, "sim.cpu0.instructions.committed"),
        "monotonic",
    );
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
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_memory() {
    let vector_stats = in_order_pipeline_payload_stats_with_max_tick(
        "in-order-vector-unit-stride-masked-load-store",
        &masked_unit_stride_vector_memory_program(),
        240,
    );

    assert_eq!(
        stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
        42,
        "masked unit-stride vector memory should retire through the success ecall\nstats:\n{vector_stats}"
    );
}

#[test]
fn rem6_run_in_order_pipeline_models_masked_vector_unit_stride_memory_element_widths() {
    for (name, program, committed) in [
        (
            "e8",
            masked_unit_stride_vector_memory_width_program(
                0xc0,
                8,
                0b000,
                1,
                &[true, false, true, false, true, false, true, false],
                &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88],
                &[0xa1, 0xb2, 0xc3, 0xd4, 0xe5, 0xf6, 0x17, 0x28],
                &[0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58],
            ),
            30,
        ),
        (
            "e16",
            masked_unit_stride_vector_memory_width_program(
                0xc8,
                4,
                0b101,
                2,
                &[true, false, true, false],
                &[0x1111, 0x2222, 0x3333, 0x4444],
                &[0xa1a2, 0xb1b2, 0xc1c2, 0xd1d2],
                &[0x5151, 0x5252, 0x5353, 0x5454],
            ),
            30,
        ),
        (
            "e64",
            masked_unit_stride_vector_memory_width_program(
                0xd8,
                2,
                0b111,
                8,
                &[true, false],
                &[0x1111_1111_1111_1111, 0x2222_2222_2222_2222],
                &[0xa1a2_a3a4_a5a6_a7a8, 0xb1b2_b3b4_b5b6_b7b8],
                &[0x5151_5151_5151_5151, 0x5252_5252_5252_5252],
            ),
            42,
        ),
    ] {
        let vector_stats = in_order_pipeline_payload_stats_with_max_tick(
            &format!("in-order-vector-unit-stride-masked-load-store-{name}"),
            &program,
            240,
        );

        assert_eq!(
            stat_value(&vector_stats, "sim.cpu0.instructions.committed"),
            committed,
            "{name} masked unit-stride vector memory should retire through the success ecall\nstats:\n{vector_stats}"
        );
    }
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
    assert!(stdout.contains("\"checker\":{\"checked_instructions\":2,\"mismatches\":0}"));
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
    in_order_pipeline_payload_stats_with_optional_memory_system(
        name,
        program,
        max_tick,
        Some("direct"),
    )
}

fn in_order_pipeline_payload_stats_with_default_memory_system(
    name: &str,
    program: &[u8],
    max_tick: u64,
) -> String {
    in_order_pipeline_payload_stats_with_optional_memory_system(name, program, max_tick, None)
}

fn in_order_pipeline_payload_stats_with_optional_memory_system(
    name: &str,
    program: &[u8],
    max_tick: u64,
    memory_system: Option<&str>,
) -> String {
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary(name, &elf);
    let max_tick = max_tick.to_string();

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
    assert!((1..=8).contains(&data_words));
    assert!(data_offset_bytes > 0 && data_offset_bytes % 4 == 0);
    let data_bytes = data_words as i32 * 4;
    let fail_instruction_index = 7 + data_words as i32 * 3 + 1;

    let mut words = vec![
        u_type(0, 10, 0x17),                            // auipc x10, 0
        i_type(data_offset_bytes, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(data_bytes, 10, 0b000, 16, 0x13),        // addi x16, x10, dest
        i_type(data_words as i32, 0, 0b000, 11, 0x13),  // addi x11, x0, vl
        vsetvli_type(0xd1, 11, 5),                      // vsetvli x5, x11, e32, m2, ta, ma
        load,
        0x0208_6127, // vse32.v v2, (x16)
    ];

    for word_index in 0..data_words {
        let offset = (word_index * 4) as i32;
        words.push(i_type(offset, 16, 0b010, 12, 0x03)); // lw x12, dest+i
        words.push(i_type(offset, 10, 0b010, 13, 0x03)); // lw x13, source+i
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
    for word_index in 0..data_words {
        let value = 0x0102_0304_u32.wrapping_add((word_index as u32) * 0x1010_1010);
        program.extend_from_slice(&value.to_le_bytes());
    }
    program.extend(std::iter::repeat_n(0, data_words * 4));
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
    )
}

fn masked_sparse_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [0, 12],
        [0xa1a2_a3a4, 0x5151_5151, 0x5252_5252, 0xb1b2_b3b4],
    )
}

fn masked_leading_gap_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [4, 12],
        [0x5151_5151, 0xa1a2_a3a4, 0x5252_5252, 0xb1b2_b3b4],
    )
}

fn masked_reversed_indexed_e32_m1_vector_memory_program() -> Vec<u8> {
    masked_indexed_e32_m1_vector_memory_program_with_offsets(
        [12, 0],
        [0x5151_5151, 0x6161_6161, 0x6262_6262, 0xa1a2_a3a4],
    )
}

fn masked_indexed_e32_m1_vector_memory_program_with_offsets(
    offsets: [u32; 2],
    source_span: [u32; 4],
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
    assert_eq!(offsets[0] % 4, 0);
    let active_word_index = usize::try_from(offsets[0] / 4).unwrap();
    let active_value = source_span[active_word_index];
    let mut expected_load = initial_vector;
    expected_load[0] = active_value;
    let mut expected_store = initial_store;
    expected_store[active_word_index] = active_value;

    let mut program = riscv64_program(&program_words);
    for word in offsets
        .into_iter()
        .chain([0xeeee_eeee, 0xeeee_eeee])
        .chain([0, 1, 0xeeee_eeee, 0xeeee_eeee])
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

fn masked_unit_stride_vector_memory_program() -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;
    const WORDS_PER_VECTOR: usize = 4;
    const VECTOR_BYTES: i32 = (WORDS_PER_VECTOR * 4) as i32;

    let fail_instruction_index = 42;
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
        vsetvli_type(0xd0, 11, 5),                           // vsetvli x5, x11, e32, m1, ta, ma
        vector_unit_stride_load_type(true, 0b110, 12, 1),    // vle32.v v1, (x12)
        vector_vi_type(0b011000, 1, 0, 0),                   // vmseq.vi v0, v1, 0
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
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let vectors: [[u32; WORDS_PER_VECTOR]; 7] = [
        [0, 1, 0, 1],
        [0x1111_1111, 0x2222_2222, 0x3333_3333, 0x4444_4444],
        [0xa1a2_a3a4, 0xb1b2_b3b4, 0xc1c2_c3c4, 0xd1d2_d3d4],
        [0, 0, 0, 0],
        [0x5151_5151, 0x5252_5252, 0x5353_5353, 0x5454_5454],
        [0xa1a2_a3a4, 0x2222_2222, 0xc1c2_c3c4, 0x4444_4444],
        [0xa1a2_a3a4, 0x5252_5252, 0xc1c2_c3c4, 0x5454_5454],
    ];

    let mut program = riscv64_program(&words);
    for vector in vectors {
        for word in vector {
            program.extend_from_slice(&word.to_le_bytes());
        }
    }
    program
}

fn masked_unit_stride_vector_memory_width_program(
    vtype: u32,
    avl: i32,
    width: u32,
    element_bytes: usize,
    active_lanes: &[bool],
    initial_lanes: &[u64],
    source_lanes: &[u64],
    store_lanes: &[u64],
) -> Vec<u8> {
    const DATA_OFFSET_BYTES: i32 = 256;

    assert_eq!(active_lanes.len(), initial_lanes.len());
    assert_eq!(active_lanes.len(), source_lanes.len());
    assert_eq!(active_lanes.len(), store_lanes.len());
    assert!(element_bytes.is_power_of_two());
    assert!(element_bytes <= 8);

    let byte_len = active_lanes.len() * element_bytes;
    assert_eq!(byte_len % 4, 0);
    let vector_bytes = byte_len as i32;
    let compare_words = byte_len / 4;
    let fail_instruction_index = 18 + compare_words as i32 * 6;

    let mut words = vec![
        u_type(0, 10, 0x17),                            // auipc x10, 0
        i_type(DATA_OFFSET_BYTES, 10, 0b000, 10, 0x13), // addi x10, x10, data
        i_type(0, 10, 0b000, 12, 0x13),                 // addi x12, x10, mask data
        i_type(vector_bytes, 10, 0b000, 13, 0x13),      // addi x13, x10, initial vector
        i_type(vector_bytes * 2, 10, 0b000, 14, 0x13),  // addi x14, x10, source vector
        i_type(vector_bytes * 3, 10, 0b000, 15, 0x13),  // addi x15, x10, load result
        i_type(vector_bytes * 4, 10, 0b000, 16, 0x13),  // addi x16, x10, store result
        i_type(vector_bytes * 5, 10, 0b000, 19, 0x13),  // addi x19, x10, expected load result
        i_type(vector_bytes * 6, 10, 0b000, 20, 0x13),  // addi x20, x10, expected store result
        i_type(avl, 0, 0b000, 11, 0x13),                // addi x11, x0, vl
        vsetvli_type(vtype, 11, 5),                     // vsetvli x5, x11, e*, m1, ta, ma
        vector_unit_stride_load_type(true, width, 12, 1),
        vector_vi_type(0b011000, 1, 0, 0), // vmseq.vi v0, v1, 0
        vector_unit_stride_load_type(true, width, 13, 2),
        vector_unit_stride_load_type(false, width, 14, 2),
        vector_unit_stride_store_type(true, width, 15, 2),
        vector_unit_stride_store_type(false, width, 16, 2),
    ];

    for word_index in 0..compare_words {
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

    for word_index in 0..compare_words {
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
    while words.len() * 4 < DATA_OFFSET_BYTES as usize {
        words.push(0);
    }

    let mask_lanes: Vec<u64> = active_lanes
        .iter()
        .copied()
        .map(|active| if active { 0 } else { 1 })
        .collect();
    let expected_load_lanes: Vec<u64> = active_lanes
        .iter()
        .zip(initial_lanes)
        .zip(source_lanes)
        .map(|((active, initial), source)| if *active { *source } else { *initial })
        .collect();
    let expected_store_lanes: Vec<u64> = active_lanes
        .iter()
        .zip(store_lanes)
        .zip(source_lanes)
        .map(|((active, store), source)| if *active { *source } else { *store })
        .collect();

    let blocks = [
        vector_lanes_to_bytes(element_bytes, &mask_lanes),
        vector_lanes_to_bytes(element_bytes, initial_lanes),
        vector_lanes_to_bytes(element_bytes, source_lanes),
        vec![0; byte_len],
        vector_lanes_to_bytes(element_bytes, store_lanes),
        vector_lanes_to_bytes(element_bytes, &expected_load_lanes),
        vector_lanes_to_bytes(element_bytes, &expected_store_lanes),
    ];

    let mut program = riscv64_program(&words);
    for bytes in blocks {
        assert_eq!(bytes.len(), byte_len);
        program.extend_from_slice(&bytes);
    }
    program
}

fn vector_lanes_to_bytes(element_bytes: usize, lanes: &[u64]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(element_bytes * lanes.len());
    for lane in lanes {
        bytes.extend_from_slice(&lane.to_le_bytes()[..element_bytes]);
    }
    bytes
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

fn in_order_pipeline_stats_for_width(path: &std::path::Path, width: u64) -> String {
    let width = width.to_string();
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
            "12",
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
            "21",
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
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.condPredicted\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.condPredictedTaken\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.condIncorrect\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.condPredicted\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.condPredictedTaken\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.condIncorrect\""));
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
fn rem6_run_stats_emit_conditional_branch_predicted_taken_from_execution() {
    let program = selected_branch_predictor_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-conditional-branch-predicted-taken", &elf);

    let stdout = selected_branch_predictor_stdout(&path, "basic");
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
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.condPredictedTaken\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.BTBLookups\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.btb.lookups::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.BTBHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.btb.misses::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.BTBUpdates\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.btb.updates::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.btb.evictions\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.BTBHitRatio\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.BTBMispredicted\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.btb.mispredict::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.predTakenBTBMiss\""));
    assert!(
        !stdout.contains("\"path\":\"system.cpu.branchPred.mispredictDueToBTBMiss_0::DirectCond\"")
    );
    assert!(!stdout
        .contains("\"path\":\"system.cpu.branchPred.mispredictDueToPredictor_0::DirectCond\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.committed_0::DirectCond\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.mispredicted_0::DirectCond\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.corrected_0::DirectCond\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.lookups_0::DirectCond\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.targetWrong_0::DirectCond\""));
    assert!(!stdout.contains("\"path\":\"system.cpu.branchPred.targetProvider_0::BTB\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.condPredictedTaken\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.BTBLookups\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.btb.lookups::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.BTBHits\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.btb.misses::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.BTBUpdates\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.btb.updates::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.btb.evictions\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.BTBHitRatio\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.BTBMispredicted\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.btb.mispredict::total\""));
    assert!(!stdout.contains("\"path\":\"system.cpu0.branchPred.predTakenBTBMiss\""));
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
            "21",
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
    let tage_program = tage_sc_l_repeated_not_taken_training_program();
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
            assert!(
                rollback_count > 0,
                "{predictor} should expose selected younger cleanup\n{stdout}"
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
    let program = tage_sc_l_repeated_not_taken_training_program();
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    let path = temp_binary("in-order-tage-sc-l-training-feedback", &elf);

    let tage_sc_l = selected_branch_predictor_stdout(&path, "tage-sc-l");

    let predictions = json_u64_field(&tage_sc_l, "\"branch_speculation_predictions\":");
    let repairs = json_u64_field(&tage_sc_l, "\"branch_speculation_repairs\":");

    assert_eq!(predictions, 4, "{tage_sc_l}");
    assert_eq!(repairs, 2, "{tage_sc_l}");
    assert!(tage_sc_l.contains("\"x5\":\"0x7\""));
    assert!(!tage_sc_l.contains("\"x6\":\"0x1\""));
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
        "160",
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

fn tage_sc_l_repeated_not_taken_training_program() -> Vec<u8> {
    riscv64_program(&[
        i_type(0, 0, 0x0, 8, 0x13),    // addi x8, x0, 0
        i_type(0, 0, 0x0, 9, 0x13),    // addi x9, x0, 0
        i_type(2, 0, 0x0, 10, 0x13),   // addi x10, x0, 2
        b_type(20, 9, 8, 0x1),         // bne x8, x9, wrong_path
        i_type(-1, 10, 0x0, 10, 0x13), // addi x10, x10, -1
        b_type(-8, 0, 10, 0x1),        // bne x10, x0, loop
        0x0070_0293,                   // addi x5, x0, 7
        0x0000_0073,                   // ecall
        0x0010_0313,                   // addi x6, x0, 1
        0x0000_0073,                   // ecall
    ])
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

fn tagged_next_line_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(32, 2, 0x3, 6, 0x03),                 // ld x6, 32(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 96, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 32..DATA_OFFSET + 40]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn page_crossing_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 0x1000;

    let mut program = riscv64_program(&[
        u_type(0x1000, 2, 0x17),    // auipc x2, 0x1000
        i_type(0, 2, 0x3, 5, 0x03), // ld x5, 0(x2)
        0x0000_0073,                // ecall
    ]);
    program.resize(DATA_OFFSET + 64, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0ff0, 0x8000_0ff0, &program);
    temp_binary(name, &elf)
}

fn useful_span_page_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 0xfe0;

    let mut program = riscv64_program(&[
        u_type(0x1000, 2, 0x17),      // auipc x2, 0x1000
        i_type(-32, 2, 0x0, 2, 0x13), // addi x2, x2, -32
        i_type(0, 2, 0x3, 5, 0x03),   // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03),  // ld x6, 16(x2)
        0x0000_0073,                  // ecall
    ]);
    program.resize(DATA_OFFSET + 64, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 16..DATA_OFFSET + 24]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn same_line_data_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(8, 2, 0x3, 6, 0x03),                  // ld x6, 8(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 32, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 8..DATA_OFFSET + 16]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn useful_data_prefetch_binary(name: &str) -> std::path::PathBuf {
    const DATA_OFFSET: usize = 64;

    let mut program = riscv64_program(&[
        u_type(0, 2, 0x17),                          // auipc x2, 0
        i_type(DATA_OFFSET as i32, 2, 0x0, 2, 0x13), // addi x2, x2, data offset
        i_type(0, 2, 0x3, 5, 0x03),                  // ld x5, 0(x2)
        i_type(16, 2, 0x3, 6, 0x03),                 // ld x6, 16(x2)
        0x0000_0073,                                 // ecall
    ]);
    program.resize(DATA_OFFSET + 48, 0);
    program[DATA_OFFSET..DATA_OFFSET + 8].copy_from_slice(&0x1122_3344_5566_7788u64.to_le_bytes());
    program[DATA_OFFSET + 16..DATA_OFFSET + 24]
        .copy_from_slice(&0x99aa_bbcc_ddee_ff00u64.to_le_bytes());
    let elf = riscv64_elf(0x8000_0000, 0x8000_0000, &program);
    temp_binary(name, &elf)
}

fn useful_instruction_prefetch_binary(name: &str) -> std::path::PathBuf {
    let mut program = riscv64_program(&[
        i_type(1, 0, 0x0, 5, 0x13), // addi x5, x0, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        i_type(1, 5, 0x0, 5, 0x13), // addi x5, x5, 1
        0x0000_0073,                // ecall
    ]);
    program.resize(48, 0);
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
