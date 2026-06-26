#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6BranchPredictorCounterSummary {
    pub(crate) lookups: u64,
    pub(crate) history_updates: u64,
    pub(crate) updates: u64,
    pub(crate) squashes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6TageScLBranchPredictorCounterSummary {
    pub(crate) lookups: u64,
    pub(crate) history_updates: u64,
    pub(crate) updates: u64,
    pub(crate) repairs: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Rem6MultiperspectivePerceptronCounterSummary {
    pub(crate) lookups: u64,
    pub(crate) updates: u64,
}
