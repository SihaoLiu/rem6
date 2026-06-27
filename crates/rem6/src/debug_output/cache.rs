use crate::{
    Rem6CacheResourceHierarchySummary, Rem6CacheResourceSummary, Rem6MemoryResourceSummary,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6CacheTraceRecord {
    hierarchy: &'static str,
    level: &'static str,
    activity: u64,
    active: u64,
    cpu_responses: u64,
    directory_decisions: u64,
    dram_accesses: u64,
    bank_accepted: u64,
    bank_immediate_hits: u64,
    bank_scheduled_misses: u64,
    bank_coalesced_misses: u64,
    prefetch_identified: u64,
    prefetch_issued: u64,
    prefetch_translation_queue_enqueued: u64,
}

impl Rem6CacheTraceRecord {
    pub(crate) fn to_json(self) -> String {
        format!(
            "{{\"hierarchy\":\"{}\",\"level\":\"{}\",\"activity\":{},\"active\":{},\"cpu_responses\":{},\"directory_decisions\":{},\"dram_accesses\":{},\"bank_accepted\":{},\"bank_immediate_hits\":{},\"bank_scheduled_misses\":{},\"bank_coalesced_misses\":{},\"prefetch_identified\":{},\"prefetch_issued\":{},\"prefetch_translation_queue_enqueued\":{}}}",
            self.hierarchy,
            self.level,
            self.activity,
            self.active,
            self.cpu_responses,
            self.directory_decisions,
            self.dram_accesses,
            self.bank_accepted,
            self.bank_immediate_hits,
            self.bank_scheduled_misses,
            self.bank_coalesced_misses,
            self.prefetch_identified,
            self.prefetch_issued,
            self.prefetch_translation_queue_enqueued,
        )
    }

    pub(crate) const fn active(self) -> u64 {
        self.active
    }

    pub(crate) const fn activity(self) -> u64 {
        self.activity
    }

    pub(crate) const fn cpu_responses(self) -> u64 {
        self.cpu_responses
    }

    pub(crate) const fn directory_decisions(self) -> u64 {
        self.directory_decisions
    }

    pub(crate) const fn dram_accesses(self) -> u64 {
        self.dram_accesses
    }

    pub(crate) const fn bank_accepted(self) -> u64 {
        self.bank_accepted
    }

    pub(crate) const fn bank_immediate_hits(self) -> u64 {
        self.bank_immediate_hits
    }

    pub(crate) const fn bank_scheduled_misses(self) -> u64 {
        self.bank_scheduled_misses
    }

    pub(crate) const fn bank_coalesced_misses(self) -> u64 {
        self.bank_coalesced_misses
    }

    pub(crate) const fn prefetch_identified(self) -> u64 {
        self.prefetch_identified
    }

    pub(crate) const fn prefetch_issued(self) -> u64 {
        self.prefetch_issued
    }

    pub(crate) const fn prefetch_translation_queue_enqueued(self) -> u64 {
        self.prefetch_translation_queue_enqueued
    }

    const fn has_activity(self) -> bool {
        self.activity != 0
            || self.active != 0
            || self.cpu_responses != 0
            || self.directory_decisions != 0
            || self.dram_accesses != 0
            || self.bank_accepted != 0
            || self.bank_immediate_hits != 0
            || self.bank_scheduled_misses != 0
            || self.bank_coalesced_misses != 0
            || self.prefetch_identified != 0
            || self.prefetch_issued != 0
            || self.prefetch_translation_queue_enqueued != 0
    }
}

pub(crate) fn cache_trace_records(
    resources: &Rem6MemoryResourceSummary,
) -> Vec<Rem6CacheTraceRecord> {
    let mut records = Vec::with_capacity(6);
    push_cache_hierarchy_records(&mut records, "instruction", &resources.cache_instruction);
    push_cache_hierarchy_records(&mut records, "data", &resources.cache_data);
    records
}

fn push_cache_hierarchy_records(
    records: &mut Vec<Rem6CacheTraceRecord>,
    hierarchy: &'static str,
    summary: &Rem6CacheResourceHierarchySummary,
) {
    for (level, summary) in [
        ("l1", &summary.l1),
        ("l2", &summary.l2),
        ("l3", &summary.l3),
    ] {
        let record = cache_trace_record(hierarchy, level, summary);
        if record.has_activity() {
            records.push(record);
        }
    }
}

fn cache_trace_record(
    hierarchy: &'static str,
    level: &'static str,
    summary: &Rem6CacheResourceSummary,
) -> Rem6CacheTraceRecord {
    Rem6CacheTraceRecord {
        hierarchy,
        level,
        activity: summary.activity,
        active: summary.active,
        cpu_responses: summary.cpu_responses,
        directory_decisions: summary.directory_decisions,
        dram_accesses: summary.dram_accesses,
        bank_accepted: summary.bank_accepted,
        bank_immediate_hits: summary.bank_immediate_hits,
        bank_scheduled_misses: summary.bank_scheduled_misses,
        bank_coalesced_misses: summary.bank_coalesced_misses,
        prefetch_identified: summary.prefetch_identified,
        prefetch_issued: summary.prefetch_issued,
        prefetch_translation_queue_enqueued: summary.prefetch_translation_queue_enqueued,
    }
}

pub(crate) fn cache_trace_active_scope_count(records: &[Rem6CacheTraceRecord]) -> u64 {
    records.iter().filter(|record| record.active() != 0).count() as u64
}

pub(crate) fn cache_trace_sum(
    records: &[Rem6CacheTraceRecord],
    value: impl Fn(Rem6CacheTraceRecord) -> u64,
) -> u64 {
    records
        .iter()
        .copied()
        .map(value)
        .fold(0u64, |acc, value| acc.saturating_add(value))
}
