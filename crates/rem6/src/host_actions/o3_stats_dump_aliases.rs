use rem6_cpu::BranchTargetKind;
use rem6_stats::StatSample;

use super::Rem6HostStatsDumpSampleSummary;
use crate::o3_branch_mismatch_aliases::{
    O3_BRANCH_MISMATCH_KIND_ALIASES, O3_BRANCH_MISMATCH_SCALAR_ALIASES,
};

pub(super) fn samples_with_gem5_aliases(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
) -> Vec<Rem6HostStatsDumpSampleSummary> {
    let mut samples = record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
        .map(Rem6HostStatsDumpSampleSummary::from_sample)
        .collect();
    append_o3_stats_dump_rate_alias_samples(record_samples, active_o3_cpus, &mut samples);
    append_o3_stats_dump_phase_alias_samples(record_samples, active_o3_cpus, &mut samples);
    append_o3_stats_dump_iew_total_bucket_alias_samples(
        record_samples,
        active_o3_cpus,
        &mut samples,
    );
    append_o3_stats_dump_inst_type_bucket_alias_samples(
        record_samples,
        active_o3_cpus,
        &mut samples,
    );
    append_o3_stats_dump_lsq_count_bucket_alias_samples(
        record_samples,
        active_o3_cpus,
        &mut samples,
    );
    append_o3_stats_dump_branch_repair_bucket_alias_samples(
        record_samples,
        active_o3_cpus,
        &mut samples,
    );
    append_o3_stats_dump_branch_mismatch_alias_samples(
        record_samples,
        active_o3_cpus,
        &mut samples,
    );
    append_o3_stats_dump_ftq_bucket_alias_samples(record_samples, active_o3_cpus, &mut samples);
    append_o3_stats_dump_frontend_alias_samples(record_samples, active_o3_cpus, &mut samples);
    append_o3_stats_dump_lsq_data_response_alias_samples(
        record_samples,
        active_o3_cpus,
        &mut samples,
    );
    samples
}

fn append_o3_stats_dump_rate_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_rate_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.iew.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_core_count(record_samples: &[StatSample], active_o3_cpus: &[u32]) -> u64 {
    record_samples
        .iter()
        .find(|sample| sample.path() == "sim.cores")
        .map(StatSample::value)
        .unwrap_or_else(|| {
            active_o3_cpus
                .iter()
                .copied()
                .max()
                .map_or(1, |cpu| u64::from(cpu) + 1)
        })
}

fn o3_stats_dump_alias_prefix(core_count: u64, cpu: u32) -> String {
    if core_count == 1 && cpu == 0 {
        "system.cpu".to_string()
    } else {
        format!("system.cpu{cpu}")
    }
}

fn o3_stats_dump_rate_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.iew.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;
    let suffix = match suffix {
        "writeback_rate_ppm" => "wbRate",
        "producer_consumer_fanout_ppm" => "wbFanout",
        _ => return None,
    };
    Some((cpu, suffix))
}

fn append_o3_stats_dump_phase_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_phase_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.iew.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_phase_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.event_summary.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;
    let suffix = match suffix {
        "issue_to_writeback_ticks" => "issueToWritebackTicks",
        "writeback_to_commit_ticks" => "writebackToCommitTicks",
        "issue_to_commit_ticks" => "issueToCommitTicks",
        _ => return None,
    };
    Some((cpu, suffix))
}

fn append_o3_stats_dump_iew_total_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_path) = o3_stats_dump_iew_total_bucket_alias_path(sample.path()) else {
            continue;
        };
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_iew_total_bucket_alias_path(path: &str) -> Option<String> {
    let (prefix, suffix) = path.split_once(".iew.")?;
    if !is_o3_stats_dump_cpu_alias_prefix(prefix) {
        return None;
    }
    match suffix {
        "instsToCommit.total"
        | "writebackCount.total"
        | "producerInst.total"
        | "consumerInst.total" => Some(format!(
            "{prefix}.iew.{}::total",
            suffix.strip_suffix(".total")?
        )),
        _ => None,
    }
}

fn append_o3_stats_dump_inst_type_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_path) = o3_stats_dump_inst_type_bucket_alias_path(sample.path()) else {
            continue;
        };
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_inst_type_bucket_alias_path(path: &str) -> Option<String> {
    if let Some((prefix, op_class)) = path.split_once(".iq.issuedInstType.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            return Some(format!("{prefix}.iq.issuedInstType_0::{op_class}"));
        }
    }
    if let Some((prefix, op_class)) = path.split_once(".commit.committedInstType.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            return Some(format!("{prefix}.commit.committedInstType_0::{op_class}"));
        }
    }
    None
}

fn is_o3_stats_dump_cpu_alias_prefix(prefix: &str) -> bool {
    if prefix == "system.cpu" {
        return true;
    }
    let Some(cpu) = prefix.strip_prefix("system.cpu") else {
        return false;
    };
    !cpu.is_empty() && cpu.bytes().all(|byte| byte.is_ascii_digit())
}

fn append_o3_stats_dump_lsq_count_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_path) = o3_stats_dump_lsq_count_bucket_alias_path(sample.path()) else {
            continue;
        };
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_lsq_count_bucket_alias_path(path: &str) -> Option<String> {
    if let Some((prefix, suffix)) = path.split_once(".lsq0.operation.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            let bucket = o3_stats_dump_lsq_operation_bucket_alias(suffix)?;
            return Some(format!("{prefix}.lsq0.operation_0::{bucket}"));
        }
    }
    if let Some((prefix, suffix)) = path.split_once(".lsq0.ordering.") {
        if is_o3_stats_dump_cpu_alias_prefix(prefix) {
            let bucket = o3_stats_dump_lsq_ordering_bucket_alias(suffix)?;
            return Some(format!("{prefix}.lsq0.ordering_0::{bucket}"));
        }
    }
    None
}

fn o3_stats_dump_lsq_operation_bucket_alias(suffix: &str) -> Option<&'static str> {
    match suffix {
        "load" => Some("Load"),
        "store" => Some("Store"),
        "loadReserved" => Some("LoadReserved"),
        "storeConditional" => Some("StoreConditional"),
        "atomic" => Some("Atomic"),
        "floatLoad" => Some("FloatLoad"),
        "floatStore" => Some("FloatStore"),
        "vectorLoad" => Some("VectorLoad"),
        "vectorStore" => Some("VectorStore"),
        "total" => Some("total"),
        _ => None,
    }
}

fn o3_stats_dump_lsq_ordering_bucket_alias(suffix: &str) -> Option<&'static str> {
    match suffix {
        "acquire" => Some("Acquire"),
        "release" => Some("Release"),
        "acquireRelease" => Some("AcquireRelease"),
        "total" => Some("total"),
        _ => None,
    }
}

fn append_o3_stats_dump_branch_repair_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_branch_repair_bucket_alias_suffix(sample.path())
        else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.iew.branchRepair_0::{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_branch_repair_bucket_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    if let Some(suffix) = path.strip_prefix("system.cpu.iew.branchRepair.") {
        return Some((0, branch_repair_bucket_alias_suffix(suffix)?));
    }
    let rest = path.strip_prefix("system.cpu")?;
    let (cpu, suffix) = rest.split_once(".iew.branchRepair.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    Some((
        cpu.parse().ok()?,
        branch_repair_bucket_alias_suffix(suffix)?,
    ))
}

fn branch_repair_bucket_alias_suffix(suffix: &str) -> Option<&'static str> {
    match suffix {
        "targetlessMismatch" => Some("TargetlessMismatch"),
        "directionOnly" => Some("DirectionOnly"),
        "wrongTarget" => Some("WrongTarget"),
        "total" => Some("total"),
        _ => None,
    }
}

fn append_o3_stats_dump_branch_mismatch_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffixes)) = o3_stats_dump_branch_mismatch_alias_suffixes(sample.path())
        else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        for suffix in suffixes {
            let alias_path = format!("{alias_prefix}.iew.{suffix}");
            if samples.iter().any(|sample| sample.path == alias_path) {
                continue;
            }
            samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
                sample, alias_path,
            ));
        }
    }
}

fn o3_stats_dump_branch_mismatch_alias_suffixes(path: &str) -> Option<(u32, Vec<String>)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;

    for alias in O3_BRANCH_MISMATCH_SCALAR_ALIASES {
        if suffix == alias.source_suffix {
            return Some((
                cpu,
                vec![
                    alias.alias_suffix.to_string(),
                    format!("{}_0::total", alias.bucket_alias),
                ],
            ));
        }
    }

    for alias in O3_BRANCH_MISMATCH_KIND_ALIASES {
        let Some(kind_name) = suffix.strip_prefix(alias.source_family) else {
            continue;
        };
        let Some(kind_name) = kind_name.strip_prefix('.') else {
            continue;
        };
        for kind in BranchTargetKind::ALL {
            if kind.canonical_stat_name() == kind_name {
                return Some((
                    cpu,
                    vec![format!(
                        "{}_0::{}",
                        alias.alias_family,
                        kind.gem5_branch_type_name()
                    )],
                ));
            }
        }
    }
    None
}

fn append_o3_stats_dump_ftq_bucket_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_ftq_bucket_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.ftq.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_ftq_bucket_alias_suffix(path: &str) -> Option<(u32, String)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.branch_event.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let cpu = cpu.parse().ok()?;
    for (source_family, source_total, alias_family) in [
        ("squash_kind", "squashes", "squashes"),
        (
            "squashed_target_kind",
            "squashed_targets",
            "squashedTargets",
        ),
        (
            "squashed_target_link_write_kind",
            "squashed_targets_with_link_writes",
            "squashedTargetsWithLinkWrites",
        ),
        (
            "squashed_target_without_link_write_kind",
            "squashed_targets_without_link_writes",
            "squashedTargetsWithoutLinkWrites",
        ),
    ] {
        if suffix == source_total {
            return Some((cpu, format!("{alias_family}_0::total")));
        }
        let Some((family, kind_name)) = suffix.split_once('.') else {
            continue;
        };
        if family != source_family {
            continue;
        }
        for kind in BranchTargetKind::ALL {
            if kind.canonical_stat_name() == kind_name {
                return Some((
                    cpu,
                    format!("{alias_family}_0::{}", kind.gem5_branch_type_name()),
                ));
            }
        }
    }
    None
}

fn append_o3_stats_dump_frontend_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some((cpu, suffix)) = o3_stats_dump_frontend_alias_suffix(sample.path()) else {
            continue;
        };
        let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu);
        let alias_path = format!("{alias_prefix}.{suffix}");
        if samples.iter().any(|sample| sample.path == alias_path) {
            continue;
        }
        samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
            sample, alias_path,
        ));
    }
}

fn o3_stats_dump_frontend_alias_suffix(path: &str) -> Option<(u32, &'static str)> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.branch_event.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let suffix = match suffix {
        "predicted_taken" => "fetch.predictedBranches",
        "mispredictions" => "bac.branchMisspredict",
        _ => return None,
    };
    Some((cpu.parse().ok()?, suffix))
}

fn append_o3_stats_dump_lsq_data_response_alias_samples(
    record_samples: &[StatSample],
    active_o3_cpus: &[u32],
    samples: &mut Vec<Rem6HostStatsDumpSampleSummary>,
) {
    let core_count = o3_stats_dump_core_count(record_samples, active_o3_cpus);
    for sample in record_samples
        .iter()
        .filter(|sample| stats_dump_sample_is_active(sample, active_o3_cpus))
    {
        let Some(alias_paths) =
            o3_stats_dump_lsq_data_response_alias_paths(core_count, sample.path())
        else {
            continue;
        };
        for alias_path in alias_paths {
            if samples.iter().any(|sample| sample.path == alias_path) {
                continue;
            }
            samples.push(Rem6HostStatsDumpSampleSummary::from_sample_with_path(
                sample, alias_path,
            ));
        }
    }
}

fn o3_stats_dump_lsq_data_response_alias_paths(core_count: u64, path: &str) -> Option<Vec<String>> {
    let rest = path.strip_prefix("sim.host_actions.stats_dump.cpu")?;
    let (cpu, suffix) = rest.split_once(".o3.")?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let alias_prefix = o3_stats_dump_alias_prefix(core_count, cpu.parse().ok()?);
    if let Some(metric_suffix) = suffix.strip_prefix("lsq_data_latency_") {
        let metric = o3_stats_dump_lsq_data_response_metric_alias(metric_suffix)?;
        return Some(vec![
            format!("{alias_prefix}.lsq0.dataResponse.{metric}"),
            format!("{alias_prefix}.lsq0.operation.total.dataResponse.{metric}"),
        ]);
    }

    let suffix = suffix.strip_prefix("lsq_operation.")?;
    let (operation, metric_suffix) =
        o3_stats_dump_lsq_data_response_operation_metric_suffix(suffix)?;
    let operation_alias = o3_stats_dump_lsq_operation_alias(operation)?;
    let metric = o3_stats_dump_lsq_data_response_metric_alias(metric_suffix)?;
    Some(vec![
        format!("{alias_prefix}.lsq0.dataResponse.{operation_alias}.{metric}"),
        format!("{alias_prefix}.lsq0.operation.{operation_alias}.dataResponse.{metric}"),
    ])
}

fn o3_stats_dump_lsq_data_response_operation_metric_suffix(suffix: &str) -> Option<(&str, &str)> {
    if let Some((operation, metric_suffix)) = suffix.split_once("_latency_") {
        return Some((operation, metric_suffix));
    }
    let (operation, metric_suffix) = suffix.split_once(".latency.")?;
    Some((operation, metric_suffix))
}

fn o3_stats_dump_lsq_data_response_metric_alias(suffix: &str) -> Option<&'static str> {
    match suffix {
        "samples" => Some("samples"),
        "ticks" => Some("totalLatency"),
        "max_ticks" => Some("maxLatency"),
        "min_ticks" => Some("minLatency"),
        "avg_ticks" => Some("avgLatency"),
        _ => None,
    }
}

fn o3_stats_dump_lsq_operation_alias(operation: &str) -> Option<&'static str> {
    match operation {
        "load" => Some("load"),
        "store" => Some("store"),
        "load_reserved" => Some("loadReserved"),
        "store_conditional" => Some("storeConditional"),
        "atomic" => Some("atomic"),
        "float_load" => Some("floatLoad"),
        "float_store" => Some("floatStore"),
        "vector_load" => Some("vectorLoad"),
        "vector_store" => Some("vectorStore"),
        _ => None,
    }
}

fn stats_dump_sample_is_active(sample: &StatSample, active_o3_cpus: &[u32]) -> bool {
    let path = sample.path().to_string();
    if is_single_cpu_o3_alias_path(&path) {
        return !active_o3_cpus.is_empty();
    }
    let Some(cpu) = o3_stats_dump_sample_cpu(&path) else {
        return true;
    };
    active_o3_cpus.contains(&cpu)
}

fn is_single_cpu_o3_alias_path(path: &str) -> bool {
    [
        "system.cpu.rob.",
        "system.cpu.rename.",
        "system.cpu.iew.",
        "system.cpu.lsq0.",
        "system.cpu.iq.",
        "system.cpu.commit.",
        "system.cpu.ftq.",
    ]
    .into_iter()
    .any(|prefix| path.starts_with(prefix))
}

fn o3_stats_dump_sample_cpu(path: &str) -> Option<u32> {
    if let Some(rest) = path.strip_prefix("sim.host_actions.stats_dump.cpu") {
        return parse_o3_stats_dump_cpu(rest, ".o3.");
    }
    let rest = path.strip_prefix("system.cpu")?;
    [
        ".rob.", ".rename.", ".iew.", ".lsq0.", ".iq.", ".commit.", ".ftq.",
    ]
    .into_iter()
    .find_map(|separator| parse_o3_stats_dump_cpu(rest, separator))
}

fn parse_o3_stats_dump_cpu(rest: &str, separator: &str) -> Option<u32> {
    let (cpu, _suffix) = rest.split_once(separator)?;
    if cpu.is_empty() || !cpu.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    cpu.parse().ok()
}

#[cfg(test)]
mod tests {
    use rem6_stats::StatId;

    use super::*;

    #[test]
    fn lsq_data_response_dump_aliases_accept_nested_operation_latency_sources() {
        let aliases = o3_stats_dump_lsq_data_response_alias_paths(
            2,
            "sim.host_actions.stats_dump.cpu1.o3.lsq_operation.vector_store.latency.avg_ticks",
        )
        .expect("nested vector-store latency source should produce gem5 aliases");

        assert_eq!(
            aliases,
            [
                "system.cpu1.lsq0.dataResponse.vectorStore.avgLatency",
                "system.cpu1.lsq0.operation.vectorStore.dataResponse.avgLatency",
            ]
        );
    }

    #[test]
    fn stats_dump_aliases_preserve_family_order_and_suppress_inactive_cpus() {
        let record_samples = [
            StatSample::new(StatId::new(1), "sim.cores", "count", 2),
            StatSample::new(
                StatId::new(2),
                "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
                "ppm",
                41,
            ),
            StatSample::new(
                StatId::new(3),
                "sim.host_actions.stats_dump.cpu1.o3.iew.writeback_rate_ppm",
                "ppm",
                99,
            ),
            StatSample::new(
                StatId::new(4),
                "system.cpu0.iew.instsToCommit.total",
                "count",
                7,
            ),
            StatSample::new(
                StatId::new(5),
                "system.cpu1.iew.instsToCommit.total",
                "count",
                8,
            ),
        ];

        let samples = samples_with_gem5_aliases(&record_samples, &[0]);
        let paths = samples
            .iter()
            .map(|sample| sample.path.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            paths,
            [
                "sim.cores",
                "sim.host_actions.stats_dump.cpu0.o3.iew.writeback_rate_ppm",
                "system.cpu0.iew.instsToCommit.total",
                "system.cpu0.iew.wbRate",
                "system.cpu0.iew.instsToCommit::total",
            ]
        );
        assert_eq!(samples[3].value, 41);
        assert_eq!(samples[4].value, 7);
    }
}
