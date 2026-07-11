use std::collections::{BTreeMap, BTreeSet};

use super::{
    pipeline_redirect_cause_index, pipeline_stage_index, pipeline_stall_cause_index,
    Rem6PipelineTraceInstruction, Rem6PipelineTraceRecord, PIPELINE_REDIRECT_CAUSES,
    PIPELINE_STAGE_NAMES, PIPELINE_STALL_CAUSES,
};

const PIPELINE_BLOCK_KINDS: [&str; 2] = ["resource_blocked", "ordering_blocked"];
const RESOURCE_BLOCKED_INDEX: usize = 0;
const ORDERING_BLOCKED_INDEX: usize = 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineCorrelationTotals {
    sequences: u64,
    stall_records: u64,
    stall_cycles: u64,
}

impl PipelineCorrelationTotals {
    fn add_sequence(&mut self, backlog: PipelineBacklogTotals) {
        self.sequences = self.sequences.saturating_add(1);
        self.stall_records = self.stall_records.saturating_add(backlog.stall_records);
        self.stall_cycles = self.stall_cycles.saturating_add(backlog.stall_cycles);
    }

    const fn is_empty(self) -> bool {
        self.sequences == 0
    }

    fn json_fields(self) -> String {
        format!(
            "\"sequences\":{},\"stall_records\":{},\"stall_cycles\":{}",
            self.sequences, self.stall_records, self.stall_cycles
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct PipelineBacklogTotals {
    stall_records: u64,
    stall_cycles: u64,
}

impl PipelineBacklogTotals {
    fn add_record(&mut self, stall_cycles: u64) {
        self.stall_records = self.stall_records.saturating_add(1);
        self.stall_cycles = self.stall_cycles.saturating_add(stall_cycles);
    }

    const fn is_empty(self) -> bool {
        self.stall_records == 0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PipelineSequenceBacklog {
    stall_causes: [PipelineStallBacklog; PIPELINE_STALL_CAUSES.len()],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PipelineStallBacklog {
    totals: PipelineBacklogTotals,
    blocked_stages:
        [[PipelineBacklogTotals; PIPELINE_STAGE_NAMES.len()]; PIPELINE_BLOCK_KINDS.len()],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct PipelineStallBacklogFlushMatrix {
    totals:
        [[PipelineCorrelationTotals; PIPELINE_REDIRECT_CAUSES.len()]; PIPELINE_STALL_CAUSES.len()],
    stage_cells: [[[[[PipelineCorrelationTotals; PIPELINE_STAGE_NAMES.len()];
        PIPELINE_STAGE_NAMES.len()]; PIPELINE_BLOCK_KINDS.len()];
        PIPELINE_REDIRECT_CAUSES.len()]; PIPELINE_STALL_CAUSES.len()],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::debug_output) struct PipelineStallBacklogFlushSummary {
    aggregate: PipelineStallBacklogFlushMatrix,
    cpus: BTreeMap<u32, PipelineStallBacklogFlushMatrix>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::debug_output) struct PipelineStallBacklogFlushMetric {
    pub(in crate::debug_output) path: String,
    pub(in crate::debug_output) unit: &'static str,
    pub(in crate::debug_output) value: u64,
}

impl PipelineStallBacklogFlushSummary {
    pub(in crate::debug_output) fn from_records(records: &[Rem6PipelineTraceRecord]) -> Self {
        let mut ordered = records.iter().collect::<Vec<_>>();
        ordered.sort_by_key(|record| (record.cycle, record.cpu));

        let mut summary = Self::default();
        let mut active = BTreeMap::<(u32, u64), PipelineSequenceBacklog>::new();
        for record in ordered {
            summary.cpus.entry(record.cpu).or_default();
            summary.record_flushes(record, &mut active);
            Self::record_stalls(record, &mut active);
            Self::retain_live_sequences(record, &mut active);
        }
        summary
    }

    fn record_flushes(
        &mut self,
        record: &Rem6PipelineTraceRecord,
        active: &mut BTreeMap<(u32, u64), PipelineSequenceBacklog>,
    ) {
        let Some(flush_cause) = record.flush_cause.and_then(pipeline_redirect_cause_index) else {
            return;
        };
        for flushed in &record.flushed {
            let Some(flushed_stage) = pipeline_stage_index(flushed.stage()) else {
                continue;
            };
            let Some(backlog) = active.remove(&(record.cpu, flushed.sequence())) else {
                continue;
            };
            self.aggregate
                .record_sequence(flush_cause, flushed_stage, &backlog);
            self.cpus.entry(record.cpu).or_default().record_sequence(
                flush_cause,
                flushed_stage,
                &backlog,
            );
        }
    }

    fn record_stalls(
        record: &Rem6PipelineTraceRecord,
        active: &mut BTreeMap<(u32, u64), PipelineSequenceBacklog>,
    ) {
        let Some(stall_cause) = record.stall_cause.and_then(pipeline_stall_cause_index) else {
            return;
        };
        let mut blocked =
            BTreeMap::<u64, [[bool; PIPELINE_STAGE_NAMES.len()]; PIPELINE_BLOCK_KINDS.len()]>::new(
            );
        Self::collect_blocked_stages(
            &mut blocked,
            RESOURCE_BLOCKED_INDEX,
            &record.resource_blocked,
        );
        Self::collect_blocked_stages(
            &mut blocked,
            ORDERING_BLOCKED_INDEX,
            &record.ordering_blocked,
        );

        for (sequence, blocked_stages) in blocked {
            let backlog = &mut active
                .entry((record.cpu, sequence))
                .or_default()
                .stall_causes[stall_cause];
            backlog.totals.add_record(record.stall_cycles);
            for (block_kind, stages) in blocked_stages.into_iter().enumerate() {
                for (stage, present) in stages.into_iter().enumerate() {
                    if present {
                        backlog.blocked_stages[block_kind][stage].add_record(record.stall_cycles);
                    }
                }
            }
        }
    }

    fn collect_blocked_stages(
        blocked: &mut BTreeMap<
            u64,
            [[bool; PIPELINE_STAGE_NAMES.len()]; PIPELINE_BLOCK_KINDS.len()],
        >,
        block_kind: usize,
        instructions: &[Rem6PipelineTraceInstruction],
    ) {
        for instruction in instructions {
            let Some(stage) = pipeline_stage_index(instruction.stage()) else {
                continue;
            };
            blocked.entry(instruction.sequence()).or_default()[block_kind][stage] = true;
        }
    }

    fn retain_live_sequences(
        record: &Rem6PipelineTraceRecord,
        active: &mut BTreeMap<(u32, u64), PipelineSequenceBacklog>,
    ) {
        let live = record
            .after_in_flight
            .iter()
            .map(|instruction| instruction.sequence())
            .collect::<BTreeSet<_>>();
        active.retain(|(cpu, sequence), _| *cpu != record.cpu || live.contains(sequence));
    }

    pub(super) fn to_json(&self) -> String {
        let cpus = self
            .cpus
            .iter()
            .map(|(cpu, matrix)| format!("\"cpu{cpu}\":{}", matrix.to_json()))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"stall_cause\":{{{}}},\"cpu\":{{{cpus}}}}}",
            self.aggregate.stall_causes_json()
        )
    }

    pub(in crate::debug_output) fn metrics(&self) -> Vec<PipelineStallBacklogFlushMetric> {
        let mut metrics = Vec::new();
        self.aggregate
            .push_metrics(&mut metrics, "stall_backlog_flush");
        for (cpu, matrix) in &self.cpus {
            matrix.push_metrics(&mut metrics, &format!("cpu.cpu{cpu}.stall_backlog_flush"));
        }
        metrics
    }
}

impl PipelineStallBacklogFlushMatrix {
    fn record_sequence(
        &mut self,
        flush_cause: usize,
        flushed_stage: usize,
        backlog: &PipelineSequenceBacklog,
    ) {
        for (stall_cause, backlog) in backlog.stall_causes.iter().enumerate() {
            if backlog.totals.is_empty() {
                continue;
            }
            self.totals[stall_cause][flush_cause].add_sequence(backlog.totals);
            for block_kind in 0..PIPELINE_BLOCK_KINDS.len() {
                for blocked_stage in 0..PIPELINE_STAGE_NAMES.len() {
                    let blocked = backlog.blocked_stages[block_kind][blocked_stage];
                    if !blocked.is_empty() {
                        self.stage_cells[stall_cause][flush_cause][block_kind][blocked_stage]
                            [flushed_stage]
                            .add_sequence(blocked);
                    }
                }
            }
        }
    }

    fn to_json(&self) -> String {
        format!("{{\"stall_cause\":{{{}}}}}", self.stall_causes_json())
    }

    fn stall_causes_json(&self) -> String {
        PIPELINE_STALL_CAUSES
            .iter()
            .enumerate()
            .map(|(stall_cause, name)| {
                let flush_causes = PIPELINE_REDIRECT_CAUSES
                    .iter()
                    .enumerate()
                    .map(|(flush_cause, flush_name)| {
                        format!(
                            "\"{flush_name}\":{}",
                            self.cause_pair_json(stall_cause, flush_cause)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("\"{name}\":{{\"flush_cause\":{{{flush_causes}}}}}")
            })
            .collect::<Vec<_>>()
            .join(",")
    }

    fn cause_pair_json(&self, stall_cause: usize, flush_cause: usize) -> String {
        let block_kinds = PIPELINE_BLOCK_KINDS
            .iter()
            .enumerate()
            .filter_map(|(block_kind, name)| {
                self.block_kind_json(stall_cause, flush_cause, block_kind)
                    .map(|json| format!("\"{name}\":{json}"))
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{{},\"block_kind\":{{{block_kinds}}}}}",
            self.totals[stall_cause][flush_cause].json_fields()
        )
    }

    fn push_metrics(&self, metrics: &mut Vec<PipelineStallBacklogFlushMetric>, prefix: &str) {
        for (stall_cause, stall_name) in PIPELINE_STALL_CAUSES.iter().enumerate() {
            for (flush_cause, flush_name) in PIPELINE_REDIRECT_CAUSES.iter().enumerate() {
                let pair_prefix =
                    format!("{prefix}.stall_cause.{stall_name}.flush_cause.{flush_name}");
                self.totals[stall_cause][flush_cause].push_metrics(metrics, &pair_prefix);
                for (block_kind, block_name) in PIPELINE_BLOCK_KINDS.iter().enumerate() {
                    for (blocked_stage, blocked_name) in PIPELINE_STAGE_NAMES.iter().enumerate() {
                        for (flushed_stage, flushed_name) in PIPELINE_STAGE_NAMES.iter().enumerate()
                        {
                            let totals = self.stage_cells[stall_cause][flush_cause][block_kind]
                                [blocked_stage][flushed_stage];
                            if totals.is_empty() {
                                continue;
                            }
                            totals.push_metrics(
                                metrics,
                                &format!(
                                    "{pair_prefix}.block_kind.{block_name}.blocked_stage.{blocked_name}.flushed_stage.{flushed_name}"
                                ),
                            );
                        }
                    }
                }
            }
        }
    }

    fn block_kind_json(
        &self,
        stall_cause: usize,
        flush_cause: usize,
        block_kind: usize,
    ) -> Option<String> {
        let blocked_stages = PIPELINE_STAGE_NAMES
            .iter()
            .enumerate()
            .filter_map(|(blocked_stage, name)| {
                self.blocked_stage_json(stall_cause, flush_cause, block_kind, blocked_stage)
                    .map(|json| format!("\"{name}\":{json}"))
            })
            .collect::<Vec<_>>();
        (!blocked_stages.is_empty())
            .then(|| format!("{{\"blocked_stage\":{{{}}}}}", blocked_stages.join(",")))
    }

    fn blocked_stage_json(
        &self,
        stall_cause: usize,
        flush_cause: usize,
        block_kind: usize,
        blocked_stage: usize,
    ) -> Option<String> {
        let flushed_stages = PIPELINE_STAGE_NAMES
            .iter()
            .enumerate()
            .filter_map(|(flushed_stage, name)| {
                let totals = self.stage_cells[stall_cause][flush_cause][block_kind][blocked_stage]
                    [flushed_stage];
                (!totals.is_empty()).then(|| format!("\"{name}\":{{{}}}", totals.json_fields()))
            })
            .collect::<Vec<_>>();
        (!flushed_stages.is_empty())
            .then(|| format!("{{\"flushed_stage\":{{{}}}}}", flushed_stages.join(",")))
    }
}

impl PipelineCorrelationTotals {
    fn push_metrics(self, metrics: &mut Vec<PipelineStallBacklogFlushMetric>, prefix: &str) {
        for (suffix, unit, value) in [
            ("sequences", "Count", self.sequences),
            ("stall_records", "Count", self.stall_records),
            ("stall_cycles", "Cycle", self.stall_cycles),
        ] {
            metrics.push(PipelineStallBacklogFlushMetric {
                path: format!("{prefix}.{suffix}"),
                unit,
                value,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::super::{Rem6PipelineTraceInstruction, Rem6PipelineTraceRecord};
    use super::PipelineStallBacklogFlushSummary;

    #[test]
    fn matching_prior_backlog_preserves_exact_cause_and_stage_totals() {
        let records = [
            trace_record(
                0,
                4,
                Some("fetch_wait"),
                3,
                Vec::new(),
                vec![instruction(7, "decode")],
                Vec::new(),
                vec![instruction(7, "decode")],
            ),
            trace_record(
                0,
                5,
                None,
                0,
                Vec::new(),
                Vec::new(),
                vec![instruction(7, "fetch1")],
                Vec::new(),
            )
            .with_flush_cause("branch_prediction"),
        ];

        let json: Value = serde_json::from_str(
            &PipelineStallBacklogFlushSummary::from_records(&records).to_json(),
        )
        .unwrap();

        for (metric, expected) in [("sequences", 1), ("stall_records", 1), ("stall_cycles", 3)] {
            assert_eq!(
                json.pointer(&format!(
                    "/stall_cause/fetch_wait/flush_cause/branch_prediction/{metric}"
                ))
                .and_then(Value::as_u64),
                Some(expected)
            );
            assert_eq!(
                json.pointer(&format!(
                    "/stall_cause/fetch_wait/flush_cause/branch_prediction/block_kind/ordering_blocked/blocked_stage/decode/flushed_stage/fetch1/{metric}"
                ))
                .and_then(Value::as_u64),
                Some(expected)
            );
            assert_eq!(
                json.pointer(&format!(
                    "/cpu/cpu0/stall_cause/fetch_wait/flush_cause/branch_prediction/{metric}"
                ))
                .and_then(Value::as_u64),
                Some(expected)
            );
        }
    }

    #[test]
    fn execute_wait_backlog_correlates_with_trap_flush_and_shared_metrics() {
        let records = [
            trace_record(
                2,
                7,
                Some("execute_wait"),
                4,
                vec![instruction(11, "execute")],
                Vec::new(),
                Vec::new(),
                vec![instruction(11, "execute")],
            ),
            trace_record(
                2,
                8,
                None,
                0,
                Vec::new(),
                Vec::new(),
                vec![instruction(11, "decode")],
                Vec::new(),
            )
            .with_flush_cause("trap_redirect"),
        ];
        let summary = PipelineStallBacklogFlushSummary::from_records(&records);
        let json: Value = serde_json::from_str(&summary.to_json()).unwrap();

        for (metric, expected) in [("sequences", 1), ("stall_records", 1), ("stall_cycles", 4)] {
            let pair = format!("/stall_cause/execute_wait/flush_cause/trap_redirect/{metric}");
            assert_eq!(json.pointer(&pair).and_then(Value::as_u64), Some(expected));
            assert_eq!(
                json.pointer(&format!("/cpu/cpu2{pair}"))
                    .and_then(Value::as_u64),
                Some(expected)
            );
            assert_eq!(
                json.pointer(&format!(
                    "/stall_cause/execute_wait/flush_cause/trap_redirect/block_kind/resource_blocked/blocked_stage/execute/flushed_stage/decode/{metric}"
                ))
                .and_then(Value::as_u64),
                Some(expected)
            );
            assert_eq!(
                summary
                    .metrics()
                    .iter()
                    .find(|candidate| {
                        candidate.path
                            == format!(
                                "stall_backlog_flush.stall_cause.execute_wait.flush_cause.trap_redirect.{metric}"
                            )
                    })
                    .map(|candidate| candidate.value),
                Some(expected)
            );
        }

        assert_eq!(
            json.pointer("/stall_cause/data_wait/flush_cause/interrupt_redirect/sequences")
                .and_then(Value::as_u64),
            Some(0)
        );
    }

    #[test]
    fn cpu_identity_and_in_flight_lifetime_bound_backlog_matches() {
        let records = [
            trace_record(
                0,
                1,
                Some("data_wait"),
                2,
                vec![instruction(9, "execute")],
                Vec::new(),
                Vec::new(),
                vec![instruction(9, "execute")],
            ),
            trace_record(
                1,
                2,
                None,
                0,
                Vec::new(),
                Vec::new(),
                vec![instruction(9, "decode")],
                Vec::new(),
            )
            .with_flush_cause("branch_prediction"),
            trace_record(
                0,
                3,
                None,
                0,
                Vec::new(),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            ),
            trace_record(
                0,
                4,
                None,
                0,
                Vec::new(),
                Vec::new(),
                vec![instruction(9, "decode")],
                Vec::new(),
            )
            .with_flush_cause("branch_prediction"),
        ];

        let json: Value = serde_json::from_str(
            &PipelineStallBacklogFlushSummary::from_records(&records).to_json(),
        )
        .unwrap();

        assert_eq!(
            json.pointer("/stall_cause/data_wait/flush_cause/branch_prediction/sequences")
                .and_then(Value::as_u64),
            Some(0)
        );
    }

    #[test]
    fn same_cycle_or_prior_flush_does_not_match_later_backlog() {
        let records = [
            trace_record(
                0,
                1,
                None,
                0,
                Vec::new(),
                Vec::new(),
                vec![instruction(5, "fetch1")],
                vec![instruction(5, "fetch1")],
            )
            .with_flush_cause("branch_prediction"),
            trace_record(
                0,
                2,
                Some("fetch_wait"),
                1,
                Vec::new(),
                vec![instruction(5, "fetch1")],
                vec![instruction(5, "fetch1")],
                Vec::new(),
            )
            .with_flush_cause("branch_prediction"),
        ];

        let json: Value = serde_json::from_str(
            &PipelineStallBacklogFlushSummary::from_records(&records).to_json(),
        )
        .unwrap();

        assert_eq!(
            json.pointer("/stall_cause/fetch_wait/flush_cause/branch_prediction/sequences")
                .and_then(Value::as_u64),
            Some(0)
        );
    }

    fn instruction(sequence: u64, stage: &'static str) -> Rem6PipelineTraceInstruction {
        Rem6PipelineTraceInstruction { sequence, stage }
    }

    #[allow(clippy::too_many_arguments)]
    fn trace_record(
        cpu: u32,
        cycle: u64,
        stall_cause: Option<&'static str>,
        stall_cycles: u64,
        resource_blocked: Vec<Rem6PipelineTraceInstruction>,
        ordering_blocked: Vec<Rem6PipelineTraceInstruction>,
        flushed: Vec<Rem6PipelineTraceInstruction>,
        after_in_flight: Vec<Rem6PipelineTraceInstruction>,
    ) -> Rem6PipelineTraceRecord {
        Rem6PipelineTraceRecord {
            cpu,
            cycle,
            stall_cycles,
            stall_cause,
            flush_cause: None,
            redirect_cause: None,
            state_changed: true,
            before_in_flight: resource_blocked
                .iter()
                .chain(ordering_blocked.iter())
                .copied()
                .collect(),
            after_in_flight,
            advanced: Vec::new(),
            resource_blocked,
            ordering_blocked,
            flushed,
            branch_predictions: 0,
            branch_mispredictions: 0,
            branch_prediction_flushed: 0,
            redirect_target_pc: None,
        }
    }

    trait TraceRecordFlushCause {
        fn with_flush_cause(self, cause: &'static str) -> Self;
    }

    impl TraceRecordFlushCause for Rem6PipelineTraceRecord {
        fn with_flush_cause(mut self, cause: &'static str) -> Self {
            self.flush_cause = Some(cause);
            self.redirect_cause = Some(cause);
            self
        }
    }
}
