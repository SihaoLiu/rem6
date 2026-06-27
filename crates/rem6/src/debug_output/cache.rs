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
    prefetch_useful: u64,
    prefetch_useful_but_miss: u64,
    prefetch_unused: u64,
    prefetch_demand_mshr_misses: u64,
    prefetch_hit_in_cache: u64,
    prefetch_hit_in_mshr: u64,
    prefetch_hit_in_write_buffer: u64,
    prefetch_late: u64,
    prefetch_accuracy_ppm: Option<u64>,
    prefetch_coverage_ppm: Option<u64>,
    prefetch_span_page: u64,
    prefetch_useful_span_page: u64,
    prefetch_in_cache: u64,
    prefetch_queue_enqueued: u64,
    prefetch_queue_issued: u64,
    prefetch_queue_dropped: u64,
    prefetch_translation_queue_enqueued: u64,
    prefetch_translation_queue_issued: u64,
    prefetch_translation_queue_translated: u64,
    prefetch_translation_queue_dropped: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6CacheTraceStat {
    suffix: &'static str,
    unit: &'static str,
    value: u64,
}

impl Rem6CacheTraceStat {
    pub(crate) const fn suffix(self) -> &'static str {
        self.suffix
    }

    pub(crate) const fn unit(self) -> &'static str {
        self.unit
    }

    pub(crate) const fn value(self) -> u64 {
        self.value
    }
}

impl Rem6CacheTraceRecord {
    pub(crate) fn to_json(self) -> String {
        format!(
            "{{\"hierarchy\":\"{}\",\"level\":\"{}\",\"activity\":{},\"active\":{},\"cpu_responses\":{},\"directory_decisions\":{},\"dram_accesses\":{},\"bank_accepted\":{},\"bank_immediate_hits\":{},\"bank_scheduled_misses\":{},\"bank_coalesced_misses\":{},\"prefetch_identified\":{},\"prefetch_issued\":{},\"prefetch_useful\":{},\"prefetch_useful_but_miss\":{},\"prefetch_unused\":{},\"prefetch_demand_mshr_misses\":{},\"prefetch_hit_in_cache\":{},\"prefetch_hit_in_mshr\":{},\"prefetch_hit_in_write_buffer\":{},\"prefetch_late\":{},\"prefetch_accuracy_ppm\":{},\"prefetch_coverage_ppm\":{},\"prefetch_span_page\":{},\"prefetch_useful_span_page\":{},\"prefetch_in_cache\":{},\"prefetch_queue_enqueued\":{},\"prefetch_queue_issued\":{},\"prefetch_queue_dropped\":{},\"prefetch_translation_queue_enqueued\":{},\"prefetch_translation_queue_issued\":{},\"prefetch_translation_queue_translated\":{},\"prefetch_translation_queue_dropped\":{}}}",
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
            self.prefetch_useful,
            self.prefetch_useful_but_miss,
            self.prefetch_unused,
            self.prefetch_demand_mshr_misses,
            self.prefetch_hit_in_cache,
            self.prefetch_hit_in_mshr,
            self.prefetch_hit_in_write_buffer,
            self.prefetch_late,
            optional_u64_json(self.prefetch_accuracy_ppm),
            optional_u64_json(self.prefetch_coverage_ppm),
            self.prefetch_span_page,
            self.prefetch_useful_span_page,
            self.prefetch_in_cache,
            self.prefetch_queue_enqueued,
            self.prefetch_queue_issued,
            self.prefetch_queue_dropped,
            self.prefetch_translation_queue_enqueued,
            self.prefetch_translation_queue_issued,
            self.prefetch_translation_queue_translated,
            self.prefetch_translation_queue_dropped,
        )
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
            || self.prefetch_useful != 0
            || self.prefetch_useful_but_miss != 0
            || self.prefetch_unused != 0
            || self.prefetch_demand_mshr_misses != 0
            || self.prefetch_hit_in_cache != 0
            || self.prefetch_hit_in_mshr != 0
            || self.prefetch_hit_in_write_buffer != 0
            || self.prefetch_late != 0
            || self.prefetch_span_page != 0
            || self.prefetch_useful_span_page != 0
            || self.prefetch_in_cache != 0
            || self.prefetch_queue_enqueued != 0
            || self.prefetch_queue_issued != 0
            || self.prefetch_queue_dropped != 0
            || self.prefetch_translation_queue_enqueued != 0
            || self.prefetch_translation_queue_issued != 0
            || self.prefetch_translation_queue_translated != 0
            || self.prefetch_translation_queue_dropped != 0
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
        prefetch_useful: summary.prefetch_useful,
        prefetch_useful_but_miss: summary.prefetch_useful_but_miss,
        prefetch_unused: summary.prefetch_unused,
        prefetch_demand_mshr_misses: summary.prefetch_demand_mshr_misses,
        prefetch_hit_in_cache: summary.prefetch_hit_in_cache,
        prefetch_hit_in_mshr: summary.prefetch_hit_in_mshr,
        prefetch_hit_in_write_buffer: summary.prefetch_hit_in_write_buffer,
        prefetch_late: summary.prefetch_late,
        prefetch_accuracy_ppm: summary.prefetch_accuracy_ppm,
        prefetch_coverage_ppm: summary.prefetch_coverage_ppm,
        prefetch_span_page: summary.prefetch_span_page,
        prefetch_useful_span_page: summary.prefetch_useful_span_page,
        prefetch_in_cache: summary.prefetch_in_cache,
        prefetch_queue_enqueued: summary.prefetch_queue_enqueued,
        prefetch_queue_issued: summary.prefetch_queue_issued,
        prefetch_queue_dropped: summary.prefetch_queue_dropped,
        prefetch_translation_queue_enqueued: summary.prefetch_translation_queue_enqueued,
        prefetch_translation_queue_issued: summary.prefetch_translation_queue_issued,
        prefetch_translation_queue_translated: summary.prefetch_translation_queue_translated,
        prefetch_translation_queue_dropped: summary.prefetch_translation_queue_dropped,
    }
}

pub(crate) fn cache_trace_active_scope_count(records: &[Rem6CacheTraceRecord]) -> u64 {
    records.iter().filter(|record| record.active != 0).count() as u64
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

pub(crate) fn cache_trace_stats(records: &[Rem6CacheTraceRecord]) -> Vec<Rem6CacheTraceStat> {
    let mut stats = Vec::new();
    push_count_stat(
        &mut stats,
        "active_scopes",
        cache_trace_active_scope_count(records),
    );
    for (suffix, value) in [
        (
            "activity",
            cache_trace_sum(records, |record| record.activity),
        ),
        (
            "cpu_responses",
            cache_trace_sum(records, |record| record.cpu_responses),
        ),
        (
            "directory_decisions",
            cache_trace_sum(records, |record| record.directory_decisions),
        ),
        (
            "dram_accesses",
            cache_trace_sum(records, |record| record.dram_accesses),
        ),
        (
            "bank.accepted",
            cache_trace_sum(records, |record| record.bank_accepted),
        ),
        (
            "bank.immediate_hits",
            cache_trace_sum(records, |record| record.bank_immediate_hits),
        ),
        (
            "bank.scheduled_misses",
            cache_trace_sum(records, |record| record.bank_scheduled_misses),
        ),
        (
            "bank.coalesced_misses",
            cache_trace_sum(records, |record| record.bank_coalesced_misses),
        ),
        (
            "prefetch.identified",
            cache_trace_sum(records, |record| record.prefetch_identified),
        ),
        (
            "prefetch.issued",
            cache_trace_sum(records, |record| record.prefetch_issued),
        ),
        (
            "prefetch.useful",
            cache_trace_sum(records, |record| record.prefetch_useful),
        ),
        (
            "prefetch.useful_but_miss",
            cache_trace_sum(records, |record| record.prefetch_useful_but_miss),
        ),
        (
            "prefetch.unused",
            cache_trace_sum(records, |record| record.prefetch_unused),
        ),
        (
            "prefetch.demand_mshr_misses",
            cache_trace_sum(records, |record| record.prefetch_demand_mshr_misses),
        ),
        (
            "prefetch.hit_in_cache",
            cache_trace_sum(records, |record| record.prefetch_hit_in_cache),
        ),
        (
            "prefetch.hit_in_mshr",
            cache_trace_sum(records, |record| record.prefetch_hit_in_mshr),
        ),
        (
            "prefetch.hit_in_write_buffer",
            cache_trace_sum(records, |record| record.prefetch_hit_in_write_buffer),
        ),
        (
            "prefetch.late",
            cache_trace_sum(records, |record| record.prefetch_late),
        ),
        (
            "prefetch.span_page",
            cache_trace_sum(records, |record| record.prefetch_span_page),
        ),
        (
            "prefetch.useful_span_page",
            cache_trace_sum(records, |record| record.prefetch_useful_span_page),
        ),
        (
            "prefetch.in_cache",
            cache_trace_sum(records, |record| record.prefetch_in_cache),
        ),
        (
            "prefetch.queue.enqueued",
            cache_trace_sum(records, |record| record.prefetch_queue_enqueued),
        ),
        (
            "prefetch.queue.issued",
            cache_trace_sum(records, |record| record.prefetch_queue_issued),
        ),
        (
            "prefetch.queue.dropped",
            cache_trace_sum(records, |record| record.prefetch_queue_dropped),
        ),
        (
            "prefetch.translation_queue.enqueued",
            cache_trace_sum(records, |record| record.prefetch_translation_queue_enqueued),
        ),
        (
            "prefetch.translation_queue.issued",
            cache_trace_sum(records, |record| record.prefetch_translation_queue_issued),
        ),
        (
            "prefetch.translation_queue.translated",
            cache_trace_sum(records, |record| {
                record.prefetch_translation_queue_translated
            }),
        ),
        (
            "prefetch.translation_queue.dropped",
            cache_trace_sum(records, |record| record.prefetch_translation_queue_dropped),
        ),
    ] {
        push_count_stat(&mut stats, suffix, value);
    }
    let useful = cache_trace_sum(records, |record| record.prefetch_useful);
    let issued = cache_trace_sum(records, |record| record.prefetch_issued);
    let demand_mshr_misses = cache_trace_sum(records, |record| record.prefetch_demand_mshr_misses);
    if let Some(accuracy_ppm) = ratio_ppm(useful, issued) {
        push_ppm_stat(&mut stats, "prefetch.accuracy_ppm", accuracy_ppm);
    }
    if let Some(coverage_ppm) = ratio_ppm(useful, useful.saturating_add(demand_mshr_misses)) {
        push_ppm_stat(&mut stats, "prefetch.coverage_ppm", coverage_ppm);
    }
    stats
}

fn push_count_stat(stats: &mut Vec<Rem6CacheTraceStat>, suffix: &'static str, value: u64) {
    stats.push(Rem6CacheTraceStat {
        suffix,
        unit: "Count",
        value,
    });
}

fn push_ppm_stat(stats: &mut Vec<Rem6CacheTraceStat>, suffix: &'static str, value: u64) {
    stats.push(Rem6CacheTraceStat {
        suffix,
        unit: "Ppm",
        value,
    });
}

fn ratio_ppm(numerator: u64, denominator: u64) -> Option<u64> {
    if denominator == 0 {
        return None;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    Some(ppm.min(u128::from(u64::MAX)) as u64)
}

fn optional_u64_json(value: Option<u64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}
