#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3IewGem5Alias {
    source_suffix: &'static str,
    alias_suffix: &'static str,
    bucket_alias_suffix: Option<&'static str>,
}

impl O3IewGem5Alias {
    const fn new(source_suffix: &'static str, alias_suffix: &'static str) -> Self {
        Self {
            source_suffix,
            alias_suffix,
            bucket_alias_suffix: None,
        }
    }

    const fn with_bucket(
        source_suffix: &'static str,
        alias_suffix: &'static str,
        bucket_alias_suffix: &'static str,
    ) -> Self {
        Self {
            source_suffix,
            alias_suffix,
            bucket_alias_suffix: Some(bucket_alias_suffix),
        }
    }

    pub(crate) const fn source_suffix(&self) -> &'static str {
        self.source_suffix
    }

    pub(crate) const fn alias_suffix(&self) -> &'static str {
        self.alias_suffix
    }

    pub(crate) const fn bucket_alias_suffix(&self) -> Option<&'static str> {
        self.bucket_alias_suffix
    }
}

pub(crate) const O3_IEW_GEM5_TOTAL_ALIASES: &[O3IewGem5Alias] = &[
    O3IewGem5Alias::with_bucket(
        "iew.insts_to_commit",
        "iew.instsToCommit.total",
        "iew.instsToCommit::total",
    ),
    O3IewGem5Alias::with_bucket(
        "iew.writeback_count",
        "iew.writebackCount.total",
        "iew.writebackCount::total",
    ),
    O3IewGem5Alias::with_bucket(
        "iew.producer_inst",
        "iew.producerInst.total",
        "iew.producerInst::total",
    ),
    O3IewGem5Alias::with_bucket(
        "iew.consumer_inst",
        "iew.consumerInst.total",
        "iew.consumerInst::total",
    ),
];

pub(crate) const O3_IEW_GEM5_RATE_ALIASES: &[O3IewGem5Alias] = &[
    O3IewGem5Alias::new("iew.writeback_rate_ppm", "iew.wbRate"),
    O3IewGem5Alias::new("iew.producer_consumer_fanout_ppm", "iew.wbFanout"),
];

pub(crate) const O3_IEW_GEM5_PHASE_ALIASES: &[O3IewGem5Alias] = &[
    O3IewGem5Alias::new(
        "event_summary.issue_to_writeback_ticks",
        "iew.issueToWritebackTicks",
    ),
    O3IewGem5Alias::new(
        "event_summary.writeback_to_commit_ticks",
        "iew.writebackToCommitTicks",
    ),
    O3IewGem5Alias::new(
        "event_summary.issue_to_commit_ticks",
        "iew.issueToCommitTicks",
    ),
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn iew_alias_descriptors_have_unique_source_and_output_suffixes() {
        let mut source_suffixes = BTreeSet::new();
        let mut alias_suffixes = BTreeSet::new();

        for alias in O3_IEW_GEM5_TOTAL_ALIASES
            .iter()
            .chain(O3_IEW_GEM5_RATE_ALIASES)
            .chain(O3_IEW_GEM5_PHASE_ALIASES)
        {
            assert!(source_suffixes.insert(alias.source_suffix()));
            assert!(alias_suffixes.insert(alias.alias_suffix()));
            if let Some(bucket_alias_suffix) = alias.bucket_alias_suffix() {
                assert!(alias_suffixes.insert(bucket_alias_suffix));
            }
        }
    }

    #[test]
    fn rate_aliases_project_ppm_sources_without_bucket_aliases() {
        assert_eq!(
            O3_IEW_GEM5_RATE_ALIASES,
            [
                O3IewGem5Alias::new("iew.writeback_rate_ppm", "iew.wbRate"),
                O3IewGem5Alias::new("iew.producer_consumer_fanout_ppm", "iew.wbFanout",),
            ]
        );
    }
}
