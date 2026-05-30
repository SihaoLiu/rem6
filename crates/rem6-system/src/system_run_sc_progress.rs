use rem6_cpu::{CpuId, RiscvStoreConditionalFailureDiagnostic};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn with_store_conditional_failure_diagnostics(
        mut self,
        diagnostics: impl IntoIterator<Item = RiscvStoreConditionalFailureDiagnostic>,
    ) -> Self {
        self.store_conditional_failure_diagnostics =
            collect_store_conditional_failure_diagnostics(diagnostics);
        self
    }

    pub fn store_conditional_failure_diagnostics(
        &self,
    ) -> &[RiscvStoreConditionalFailureDiagnostic] {
        &self.store_conditional_failure_diagnostics
    }

    pub fn store_conditional_failure_diagnostic_count(&self) -> usize {
        self.store_conditional_failure_diagnostics.len()
    }

    pub fn store_conditional_failure_diagnostic_count_for_cpu(&self, cpu: CpuId) -> usize {
        self.store_conditional_failure_diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.cpu() == cpu)
            .count()
    }

    pub fn has_store_conditional_failure_diagnostics(&self) -> bool {
        !self.store_conditional_failure_diagnostics.is_empty()
    }
}

fn collect_store_conditional_failure_diagnostics<I>(
    diagnostics: I,
) -> Vec<RiscvStoreConditionalFailureDiagnostic>
where
    I: IntoIterator<Item = RiscvStoreConditionalFailureDiagnostic>,
{
    let mut diagnostics = diagnostics.into_iter().collect::<Vec<_>>();
    diagnostics.sort_by_key(|diagnostic| {
        (
            diagnostic.first_failure_tick(),
            diagnostic.last_failure_tick(),
            diagnostic.cpu(),
            diagnostic.address(),
            diagnostic.size(),
            diagnostic.failure_count(),
            diagnostic.diagnostic_threshold(),
        )
    });
    diagnostics
}
