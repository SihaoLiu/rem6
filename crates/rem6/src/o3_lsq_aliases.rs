use rem6_cpu::{O3RuntimeLsqOperation, O3RuntimeLsqOrdering};

pub(crate) const O3_LSQ_TOTAL_ALIAS: &str = "total";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LsqOperationGem5Alias {
    operation: O3RuntimeLsqOperation,
    alias: &'static str,
    bucket_alias: &'static str,
}

impl O3LsqOperationGem5Alias {
    const fn new(
        operation: O3RuntimeLsqOperation,
        alias: &'static str,
        bucket_alias: &'static str,
    ) -> Self {
        Self {
            operation,
            alias,
            bucket_alias,
        }
    }

    pub(crate) const fn operation(&self) -> O3RuntimeLsqOperation {
        self.operation
    }

    pub(crate) const fn source_name(&self) -> &'static str {
        self.operation.as_str()
    }

    pub(crate) const fn alias(&self) -> &'static str {
        self.alias
    }

    pub(crate) const fn bucket_alias(&self) -> &'static str {
        self.bucket_alias
    }
}

pub(crate) const O3_LSQ_OPERATION_GEM5_ALIASES: &[O3LsqOperationGem5Alias] = &[
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::Load, "load", "Load"),
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::Store, "store", "Store"),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::LoadReserved,
        "loadReserved",
        "LoadReserved",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::StoreConditional,
        "storeConditional",
        "StoreConditional",
    ),
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::Atomic, "atomic", "Atomic"),
    O3LsqOperationGem5Alias::new(O3RuntimeLsqOperation::FloatLoad, "floatLoad", "FloatLoad"),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::FloatStore,
        "floatStore",
        "FloatStore",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::VectorLoad,
        "vectorLoad",
        "VectorLoad",
    ),
    O3LsqOperationGem5Alias::new(
        O3RuntimeLsqOperation::VectorStore,
        "vectorStore",
        "VectorStore",
    ),
];

pub(crate) fn o3_lsq_operation_gem5_alias_by_source_name(
    source_name: &str,
) -> Option<&'static O3LsqOperationGem5Alias> {
    O3_LSQ_OPERATION_GEM5_ALIASES
        .iter()
        .find(|alias| alias.source_name() == source_name)
}

pub(crate) fn o3_lsq_operation_gem5_alias_by_alias(
    alias_name: &str,
) -> Option<&'static O3LsqOperationGem5Alias> {
    O3_LSQ_OPERATION_GEM5_ALIASES
        .iter()
        .find(|alias| alias.alias() == alias_name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LsqOrderingGem5Alias {
    ordering: O3RuntimeLsqOrdering,
    alias: &'static str,
    bucket_alias: &'static str,
}

impl O3LsqOrderingGem5Alias {
    const fn new(
        ordering: O3RuntimeLsqOrdering,
        alias: &'static str,
        bucket_alias: &'static str,
    ) -> Self {
        Self {
            ordering,
            alias,
            bucket_alias,
        }
    }

    pub(crate) const fn ordering(&self) -> O3RuntimeLsqOrdering {
        self.ordering
    }

    pub(crate) const fn alias(&self) -> &'static str {
        self.alias
    }

    pub(crate) const fn bucket_alias(&self) -> &'static str {
        self.bucket_alias
    }
}

pub(crate) const O3_LSQ_ORDERING_GEM5_ALIASES: &[O3LsqOrderingGem5Alias] = &[
    O3LsqOrderingGem5Alias::new(O3RuntimeLsqOrdering::Acquire, "acquire", "Acquire"),
    O3LsqOrderingGem5Alias::new(O3RuntimeLsqOrdering::Release, "release", "Release"),
    O3LsqOrderingGem5Alias::new(
        O3RuntimeLsqOrdering::AcquireRelease,
        "acquireRelease",
        "AcquireRelease",
    ),
];

pub(crate) fn o3_lsq_ordering_gem5_alias_by_alias(
    alias_name: &str,
) -> Option<&'static O3LsqOrderingGem5Alias> {
    O3_LSQ_ORDERING_GEM5_ALIASES
        .iter()
        .find(|alias| alias.alias() == alias_name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3LsqDataResponseMetric {
    Samples,
    Ticks,
    MaxTicks,
    MinTicks,
    AvgTicks,
}

impl O3LsqDataResponseMetric {
    const fn source_suffix(self) -> &'static str {
        match self {
            Self::Samples => "samples",
            Self::Ticks => "ticks",
            Self::MaxTicks => "max_ticks",
            Self::MinTicks => "min_ticks",
            Self::AvgTicks => "avg_ticks",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LsqDataResponseGem5Alias {
    metric: O3LsqDataResponseMetric,
    alias: &'static str,
    unit: &'static str,
}

impl O3LsqDataResponseGem5Alias {
    const fn new(metric: O3LsqDataResponseMetric, alias: &'static str, unit: &'static str) -> Self {
        Self {
            metric,
            alias,
            unit,
        }
    }

    pub(crate) const fn metric(&self) -> O3LsqDataResponseMetric {
        self.metric
    }

    pub(crate) const fn source_suffix(&self) -> &'static str {
        self.metric.source_suffix()
    }

    pub(crate) const fn alias(&self) -> &'static str {
        self.alias
    }

    pub(crate) const fn unit(&self) -> &'static str {
        self.unit
    }
}

pub(crate) const O3_LSQ_DATA_RESPONSE_GEM5_ALIASES: &[O3LsqDataResponseGem5Alias] = &[
    O3LsqDataResponseGem5Alias::new(O3LsqDataResponseMetric::Samples, "samples", "Count"),
    O3LsqDataResponseGem5Alias::new(O3LsqDataResponseMetric::Ticks, "totalLatency", "Tick"),
    O3LsqDataResponseGem5Alias::new(O3LsqDataResponseMetric::MaxTicks, "maxLatency", "Tick"),
    O3LsqDataResponseGem5Alias::new(O3LsqDataResponseMetric::MinTicks, "minLatency", "Tick"),
    O3LsqDataResponseGem5Alias::new(O3LsqDataResponseMetric::AvgTicks, "avgLatency", "Tick"),
];

pub(crate) fn o3_lsq_data_response_gem5_alias_by_source_suffix(
    source_suffix: &str,
) -> Option<&'static O3LsqDataResponseGem5Alias> {
    O3_LSQ_DATA_RESPONSE_GEM5_ALIASES
        .iter()
        .find(|alias| alias.source_suffix() == source_suffix)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn operation_aliases_match_tracked_order_and_spellings() {
        let descriptors = O3_LSQ_OPERATION_GEM5_ALIASES
            .iter()
            .map(|alias| {
                (
                    alias.operation(),
                    alias.source_name(),
                    alias.alias(),
                    alias.bucket_alias(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            descriptors,
            [
                (O3RuntimeLsqOperation::Load, "load", "load", "Load"),
                (O3RuntimeLsqOperation::Store, "store", "store", "Store"),
                (
                    O3RuntimeLsqOperation::LoadReserved,
                    "load_reserved",
                    "loadReserved",
                    "LoadReserved",
                ),
                (
                    O3RuntimeLsqOperation::StoreConditional,
                    "store_conditional",
                    "storeConditional",
                    "StoreConditional",
                ),
                (O3RuntimeLsqOperation::Atomic, "atomic", "atomic", "Atomic"),
                (
                    O3RuntimeLsqOperation::FloatLoad,
                    "float_load",
                    "floatLoad",
                    "FloatLoad",
                ),
                (
                    O3RuntimeLsqOperation::FloatStore,
                    "float_store",
                    "floatStore",
                    "FloatStore",
                ),
                (
                    O3RuntimeLsqOperation::VectorLoad,
                    "vector_load",
                    "vectorLoad",
                    "VectorLoad",
                ),
                (
                    O3RuntimeLsqOperation::VectorStore,
                    "vector_store",
                    "vectorStore",
                    "VectorStore",
                ),
            ]
        );
        assert_eq!(
            O3_LSQ_OPERATION_GEM5_ALIASES
                .iter()
                .map(O3LsqOperationGem5Alias::operation)
                .collect::<Vec<_>>(),
            O3RuntimeLsqOperation::TRACKED
        );
    }

    #[test]
    fn ordering_aliases_match_tracked_order_and_spellings() {
        let descriptors = O3_LSQ_ORDERING_GEM5_ALIASES
            .iter()
            .map(|alias| (alias.ordering(), alias.alias(), alias.bucket_alias()))
            .collect::<Vec<_>>();

        assert_eq!(
            descriptors,
            [
                (O3RuntimeLsqOrdering::Acquire, "acquire", "Acquire"),
                (O3RuntimeLsqOrdering::Release, "release", "Release"),
                (
                    O3RuntimeLsqOrdering::AcquireRelease,
                    "acquireRelease",
                    "AcquireRelease",
                ),
            ]
        );
        assert_eq!(
            O3_LSQ_ORDERING_GEM5_ALIASES
                .iter()
                .map(O3LsqOrderingGem5Alias::ordering)
                .collect::<Vec<_>>(),
            O3RuntimeLsqOrdering::TRACKED
        );
    }

    #[test]
    fn data_response_aliases_match_order_spellings_and_units() {
        let descriptors = O3_LSQ_DATA_RESPONSE_GEM5_ALIASES
            .iter()
            .map(|alias| {
                (
                    alias.metric(),
                    alias.source_suffix(),
                    alias.alias(),
                    alias.unit(),
                )
            })
            .collect::<Vec<_>>();

        assert_eq!(
            descriptors,
            [
                (
                    O3LsqDataResponseMetric::Samples,
                    "samples",
                    "samples",
                    "Count",
                ),
                (
                    O3LsqDataResponseMetric::Ticks,
                    "ticks",
                    "totalLatency",
                    "Tick",
                ),
                (
                    O3LsqDataResponseMetric::MaxTicks,
                    "max_ticks",
                    "maxLatency",
                    "Tick",
                ),
                (
                    O3LsqDataResponseMetric::MinTicks,
                    "min_ticks",
                    "minLatency",
                    "Tick",
                ),
                (
                    O3LsqDataResponseMetric::AvgTicks,
                    "avg_ticks",
                    "avgLatency",
                    "Tick",
                ),
            ]
        );
    }

    #[test]
    fn aliases_are_unique() {
        let mut source_names = BTreeSet::new();
        let mut aliases = BTreeSet::new();
        let mut bucket_aliases = BTreeSet::new();
        for descriptor in O3_LSQ_OPERATION_GEM5_ALIASES {
            assert!(source_names.insert(descriptor.source_name()));
            assert!(aliases.insert(descriptor.alias()));
            assert!(bucket_aliases.insert(descriptor.bucket_alias()));
        }

        let mut ordering_aliases = BTreeSet::new();
        let mut ordering_bucket_aliases = BTreeSet::new();
        for descriptor in O3_LSQ_ORDERING_GEM5_ALIASES {
            assert!(ordering_aliases.insert(descriptor.alias()));
            assert!(ordering_bucket_aliases.insert(descriptor.bucket_alias()));
        }

        let mut metric_sources = BTreeSet::new();
        let mut metric_aliases = BTreeSet::new();
        for descriptor in O3_LSQ_DATA_RESPONSE_GEM5_ALIASES {
            assert!(metric_sources.insert(descriptor.source_suffix()));
            assert!(metric_aliases.insert(descriptor.alias()));
        }
    }

    #[test]
    fn lookup_functions_project_expected_aliases() {
        assert_eq!(
            o3_lsq_operation_gem5_alias_by_source_name("store_conditional")
                .map(O3LsqOperationGem5Alias::alias),
            Some("storeConditional")
        );
        assert_eq!(
            o3_lsq_operation_gem5_alias_by_alias("vectorLoad")
                .map(O3LsqOperationGem5Alias::bucket_alias),
            Some("VectorLoad")
        );
        assert_eq!(
            o3_lsq_ordering_gem5_alias_by_alias("acquireRelease")
                .map(O3LsqOrderingGem5Alias::bucket_alias),
            Some("AcquireRelease")
        );
        assert_eq!(
            o3_lsq_data_response_gem5_alias_by_source_suffix("max_ticks")
                .map(O3LsqDataResponseGem5Alias::alias),
            Some("maxLatency")
        );
    }

    #[test]
    fn total_alias_is_shared_and_none_is_not_described() {
        assert_eq!(O3_LSQ_TOTAL_ALIAS, "total");
        assert!(O3_LSQ_OPERATION_GEM5_ALIASES
            .iter()
            .all(|alias| alias.operation() != O3RuntimeLsqOperation::None));
        assert!(O3_LSQ_ORDERING_GEM5_ALIASES
            .iter()
            .all(|alias| alias.ordering() != O3RuntimeLsqOrdering::None));
        assert!(o3_lsq_operation_gem5_alias_by_source_name("none").is_none());
        assert!(o3_lsq_operation_gem5_alias_by_alias("none").is_none());
        assert!(o3_lsq_ordering_gem5_alias_by_alias("none").is_none());
    }
}
