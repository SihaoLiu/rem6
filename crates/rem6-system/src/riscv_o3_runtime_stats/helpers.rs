use rem6_cpu::{
    BranchTargetKind, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeStats,
};
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::groups::{
    RiscvO3RuntimeBranchEventKindStats, RiscvO3RuntimeBranchRepairStats,
    RiscvO3RuntimeFuLatencyClassStats, RiscvO3RuntimeLsqLatencyStats,
};
pub(super) fn register_o3_counter(
    registry: &mut StatsRegistry,
    prefix: &str,
    name: &str,
    unit: &str,
) -> Result<StatId, StatsError> {
    registry.register_counter(format!("{prefix}.{name}"), unit)
}

pub(super) fn o3_branch_mispredicts(stats: O3RuntimeStats) -> u64 {
    stats.branch_repair_mispredicts()
}

pub(super) fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    ppm.min(u128::from(u64::MAX)) as u64
}

pub(super) fn o3_lsq_operation_alias(operation: O3RuntimeLsqOperation) -> &'static str {
    match operation {
        O3RuntimeLsqOperation::None => "none",
        O3RuntimeLsqOperation::Load => "load",
        O3RuntimeLsqOperation::Store => "store",
        O3RuntimeLsqOperation::LoadReserved => "loadReserved",
        O3RuntimeLsqOperation::StoreConditional => "storeConditional",
        O3RuntimeLsqOperation::Atomic => "atomic",
        O3RuntimeLsqOperation::FloatLoad => "floatLoad",
        O3RuntimeLsqOperation::FloatStore => "floatStore",
        O3RuntimeLsqOperation::VectorLoad => "vectorLoad",
        O3RuntimeLsqOperation::VectorStore => "vectorStore",
    }
}

pub(super) fn o3_lsq_ordering_alias(ordering: O3RuntimeLsqOrdering) -> &'static str {
    match ordering {
        O3RuntimeLsqOrdering::None => "none",
        O3RuntimeLsqOrdering::Acquire => "acquire",
        O3RuntimeLsqOrdering::Release => "release",
        O3RuntimeLsqOrdering::AcquireRelease => "acquireRelease",
    }
}

pub(super) fn register_o3_lsq_operation_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_operation.{}", operation.as_str()),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_operation_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq0.operation.{}", o3_lsq_operation_alias(operation)),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_operation_nested_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    suffix: &str,
    unit: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_operation.{}.{suffix}", operation.as_str()),
            unit,
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_operation_suffix_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    suffix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_operation.{}_{}", operation.as_str(), suffix),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_operation_forwarding_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    suffix: &str,
) -> Result<[StatId; O3RuntimeLsqOperation::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "lsq0.operation.{}.{}",
                o3_lsq_operation_alias(operation),
                suffix
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_latency_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    stem: &str,
) -> Result<RiscvO3RuntimeLsqLatencyStats, StatsError> {
    Ok(RiscvO3RuntimeLsqLatencyStats {
        samples: register_o3_counter(registry, prefix, &format!("{stem}_samples"), "Count")?,
        ticks: register_o3_counter(registry, prefix, &format!("{stem}_ticks"), "Tick")?,
        max_ticks: register_o3_counter(registry, prefix, &format!("{stem}_max_ticks"), "Tick")?,
        min_ticks: register_o3_counter(registry, prefix, &format!("{stem}_min_ticks"), "Tick")?,
        avg_ticks: register_o3_counter(registry, prefix, &format!("{stem}_avg_ticks"), "Tick")?,
    })
}

pub(super) fn register_o3_lsq_operation_latency_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT], StatsError> {
    let empty = RiscvO3RuntimeLsqLatencyStats {
        samples: StatId::new(0),
        ticks: StatId::new(0),
        max_ticks: StatId::new(0),
        min_ticks: StatId::new(0),
        avg_ticks: StatId::new(0),
    };
    let mut stats = [empty; O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        stats[operation.index()] = register_o3_lsq_latency_counters(
            registry,
            prefix,
            &format!("lsq_operation.{}_latency", operation.as_str()),
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_operation_nested_latency_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT], StatsError> {
    let empty = RiscvO3RuntimeLsqLatencyStats {
        samples: StatId::new(0),
        ticks: StatId::new(0),
        max_ticks: StatId::new(0),
        min_ticks: StatId::new(0),
        avg_ticks: StatId::new(0),
    };
    let mut stats = [empty; O3RuntimeLsqOperation::COUNT];
    for operation in O3RuntimeLsqOperation::TRACKED {
        let operation_name = operation.as_str();
        stats[operation.index()] = RiscvO3RuntimeLsqLatencyStats {
            samples: register_o3_counter(
                registry,
                prefix,
                &format!("lsq_operation.{operation_name}.latency.samples"),
                "Count",
            )?,
            ticks: register_o3_counter(
                registry,
                prefix,
                &format!("lsq_operation.{operation_name}.latency.ticks"),
                "Tick",
            )?,
            max_ticks: register_o3_counter(
                registry,
                prefix,
                &format!("lsq_operation.{operation_name}.latency.max_ticks"),
                "Tick",
            )?,
            min_ticks: register_o3_counter(
                registry,
                prefix,
                &format!("lsq_operation.{operation_name}.latency.min_ticks"),
                "Tick",
            )?,
            avg_ticks: register_o3_counter(
                registry,
                prefix,
                &format!("lsq_operation.{operation_name}.latency.avg_ticks"),
                "Tick",
            )?,
        };
    }
    Ok(stats)
}

pub(super) fn set_o3_lsq_latency_counters(
    registry: &mut StatsRegistry,
    stats: RiscvO3RuntimeLsqLatencyStats,
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
    avg_ticks: u64,
) -> Result<(), StatsError> {
    for (stat, value) in [
        (stats.samples, samples),
        (stats.ticks, ticks),
        (stats.max_ticks, max_ticks),
        (stats.min_ticks, min_ticks),
        (stats.avg_ticks, avg_ticks),
    ] {
        registry.set_resettable_counter(stat, value)?;
    }
    Ok(())
}

pub(super) fn register_o3_lsq_ordering_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOrdering::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOrdering::COUNT];
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        stats[ordering.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq_ordering.{}", ordering.as_str()),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_lsq_ordering_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeLsqOrdering::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeLsqOrdering::COUNT];
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        stats[ordering.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("lsq0.ordering.{}", o3_lsq_ordering_alias(ordering)),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_branch_repair_kind_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeBranchRepairStats {
        targetless_mismatch: StatId::new(0),
        wrong_target: StatId::new(0),
        direction_only: StatId::new(0),
    }; BranchTargetKind::COUNT];
    for kind in BranchTargetKind::ALL {
        let stat_name = kind.canonical_stat_name();
        stats[kind.index()] = RiscvO3RuntimeBranchRepairStats {
            targetless_mismatch: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair_targetless_mismatch_kind.{stat_name}"),
                "Count",
            )?,
            wrong_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair_wrong_target_kind.{stat_name}"),
                "Count",
            )?,
            direction_only: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair_direction_only_kind.{stat_name}"),
                "Count",
            )?,
        };
    }
    Ok(stats)
}

pub(super) fn register_o3_branch_event_kind_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeBranchEventKindStats; BranchTargetKind::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeBranchEventKindStats {
        kind: StatId::new(0),
        taken: StatId::new(0),
        not_taken: StatId::new(0),
        predicted_taken: StatId::new(0),
        predicted_not_taken: StatId::new(0),
        predicted_target: StatId::new(0),
        predicted_target_match: StatId::new(0),
        predicted_target_mismatch: StatId::new(0),
        resolved_target: StatId::new(0),
        misprediction: StatId::new(0),
        link_write: StatId::new(0),
        without_link_write: StatId::new(0),
        squash: StatId::new(0),
        squashed_target: StatId::new(0),
        squashed_target_link_write: StatId::new(0),
        squashed_target_without_link_write: StatId::new(0),
    }; BranchTargetKind::COUNT];
    for kind in BranchTargetKind::ALL {
        let stat_name = kind.canonical_stat_name();
        stats[kind.index()] = RiscvO3RuntimeBranchEventKindStats {
            kind: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.kind.{stat_name}"),
                "Count",
            )?,
            taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.taken_kind.{stat_name}"),
                "Count",
            )?,
            not_taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.not_taken_kind.{stat_name}"),
                "Count",
            )?,
            predicted_taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_taken_kind.{stat_name}"),
                "Count",
            )?,
            predicted_not_taken: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_not_taken_kind.{stat_name}"),
                "Count",
            )?,
            predicted_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_target_kind.{stat_name}"),
                "Count",
            )?,
            predicted_target_match: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_target_match_kind.{stat_name}"),
                "Count",
            )?,
            predicted_target_mismatch: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.predicted_target_mismatch_kind.{stat_name}"),
                "Count",
            )?,
            resolved_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.resolved_target_kind.{stat_name}"),
                "Count",
            )?,
            misprediction: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.misprediction_kind.{stat_name}"),
                "Count",
            )?,
            link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.link_write_kind.{stat_name}"),
                "Count",
            )?,
            without_link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.without_link_write_kind.{stat_name}"),
                "Count",
            )?,
            squash: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squash_kind.{stat_name}"),
                "Count",
            )?,
            squashed_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squashed_target_kind.{stat_name}"),
                "Count",
            )?,
            squashed_target_link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squashed_target_link_write_kind.{stat_name}"),
                "Count",
            )?,
            squashed_target_without_link_write: register_o3_counter(
                registry,
                prefix,
                &format!("branch_event.squashed_target_without_link_write_kind.{stat_name}"),
                "Count",
            )?,
        };
    }
    Ok(stats)
}

pub(super) fn register_o3_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeFuLatencyClassStats {
        instructions: StatId::new(0),
        latency_cycles: StatId::new(0),
    }; O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        let stat_stem = class.stat_stem();
        stats[class.index()] = RiscvO3RuntimeFuLatencyClassStats {
            instructions: register_o3_counter(
                registry,
                prefix,
                &format!("fu_{stat_stem}_instructions"),
                "Count",
            )?,
            latency_cycles: register_o3_counter(
                registry,
                prefix,
                &format!("fu_{stat_stem}_latency_cycles"),
                "Cycle",
            )?,
        };
    }
    Ok(stats)
}

pub(super) fn register_o3_nested_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeFuLatencyClassStats {
        instructions: StatId::new(0),
        latency_cycles: StatId::new(0),
    }; O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        let stat_stem = class.stat_stem();
        stats[class.index()] = RiscvO3RuntimeFuLatencyClassStats {
            instructions: register_o3_counter(
                registry,
                prefix,
                &format!("fu_latency_class.{stat_stem}.instructions"),
                "Count",
            )?,
            latency_cycles: register_o3_counter(
                registry,
                prefix,
                &format!("fu_latency_class.{stat_stem}.cycles"),
                "Cycle",
            )?,
        };
    }
    Ok(stats)
}

pub(super) fn register_o3_iq_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("iq.issued_inst_type.{}", o3_iq_fu_latency_class_stem(class)),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_iq_fu_latency_class_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "iq.issuedInstType.{}",
                o3_fu_latency_class_inst_type_alias(class)
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_commit_fu_latency_class_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "commit.committed_inst_type.{}",
                o3_fu_latency_class_inst_type_stem(class)
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn register_o3_commit_fu_latency_class_alias_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[StatId; O3RuntimeFuLatencyClass::COUNT], StatsError> {
    let mut stats = [StatId::new(0); O3RuntimeFuLatencyClass::COUNT];
    for class in O3RuntimeFuLatencyClass::ALL {
        stats[class.index()] = register_o3_counter(
            registry,
            prefix,
            &format!(
                "commit.committedInstType.{}",
                o3_fu_latency_class_inst_type_alias(class)
            ),
            "Count",
        )?;
    }
    Ok(stats)
}

pub(super) fn o3_iq_fu_latency_class_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    o3_fu_latency_class_inst_type_stem(class)
}

pub(super) fn o3_fu_latency_class_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

pub(super) fn o3_fu_latency_class_inst_type_alias(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "IntMult",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "IntDiv",
        O3RuntimeFuLatencyClass::ScalarFloatAdd => "FloatAdd",
        O3RuntimeFuLatencyClass::ScalarFloatCompare => "FloatCmp",
        O3RuntimeFuLatencyClass::ScalarFloatMisc => "FloatMisc",
        O3RuntimeFuLatencyClass::ScalarFloatMul => "FloatMult",
        O3RuntimeFuLatencyClass::ScalarFloatFma => "FloatMultAcc",
        O3RuntimeFuLatencyClass::ScalarFloatDiv => "FloatDiv",
        O3RuntimeFuLatencyClass::ScalarFloatSqrt => "FloatSqrt",
        O3RuntimeFuLatencyClass::VectorIntegerMul => "SimdMult",
        O3RuntimeFuLatencyClass::VectorIntegerDiv => "SimdDiv",
        O3RuntimeFuLatencyClass::VectorFloatAdd => "SimdFloatAdd",
        O3RuntimeFuLatencyClass::VectorFloatCompare => "SimdFloatCmp",
        O3RuntimeFuLatencyClass::VectorFloatMisc => "SimdFloatMisc",
        O3RuntimeFuLatencyClass::VectorFloatMul => "SimdFloatMult",
        O3RuntimeFuLatencyClass::VectorFloatFma => "SimdFloatMultAcc",
        O3RuntimeFuLatencyClass::VectorFloatDiv => "SimdFloatDiv",
        O3RuntimeFuLatencyClass::VectorFloatSqrt => "SimdFloatSqrt",
    }
}
