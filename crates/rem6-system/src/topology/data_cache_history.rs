use std::collections::BTreeMap;

use rem6_coherence::ParallelCoherenceRunHistory;

use crate::{RiscvDataCacheProtocol, RiscvDataCacheRunHistoryRecord};

use super::RiscvTopologySystem;

impl RiscvTopologySystem {
    pub fn msi_data_cache_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_runs(&self.msi_data_cache_runs())
    }

    pub fn mesi_data_cache_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_runs(&self.mesi_data_cache_runs())
    }

    pub fn moesi_data_cache_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_runs(&self.moesi_data_cache_runs())
    }

    pub fn chi_data_cache_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_runs(&self.chi_data_cache_runs())
    }

    pub fn data_cache_parallel_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_histories([
            self.msi_data_cache_run_history(),
            self.mesi_data_cache_run_history(),
            self.moesi_data_cache_run_history(),
            self.chi_data_cache_run_history(),
        ])
    }

    pub fn data_cache_parallel_run_history_for_protocol(
        &self,
        protocol: RiscvDataCacheProtocol,
    ) -> ParallelCoherenceRunHistory {
        match protocol {
            RiscvDataCacheProtocol::Msi => self.msi_data_cache_run_history(),
            RiscvDataCacheProtocol::Mesi => self.mesi_data_cache_run_history(),
            RiscvDataCacheProtocol::Moesi => self.moesi_data_cache_run_history(),
            RiscvDataCacheProtocol::Chi => self.chi_data_cache_run_history(),
        }
    }

    pub fn data_cache_parallel_run_count_for_protocol(
        &self,
        protocol: RiscvDataCacheProtocol,
    ) -> usize {
        self.data_cache_parallel_run_history_for_protocol(protocol)
            .run_count()
    }

    pub fn has_data_cache_parallel_run_history_for_protocol(
        &self,
        protocol: RiscvDataCacheProtocol,
    ) -> bool {
        self.data_cache_parallel_run_count_for_protocol(protocol) != 0
    }

    pub fn data_cache_parallel_run_histories_by_protocol(
        &self,
    ) -> BTreeMap<RiscvDataCacheProtocol, ParallelCoherenceRunHistory> {
        let mut histories = BTreeMap::new();
        for protocol in [
            RiscvDataCacheProtocol::Msi,
            RiscvDataCacheProtocol::Mesi,
            RiscvDataCacheProtocol::Moesi,
            RiscvDataCacheProtocol::Chi,
        ] {
            let history = self.data_cache_parallel_run_history_for_protocol(protocol);
            if !history.is_empty() {
                histories.insert(protocol, history);
            }
        }
        histories
    }

    pub fn data_cache_parallel_run_history_record(
        &self,
        protocol: RiscvDataCacheProtocol,
    ) -> Option<RiscvDataCacheRunHistoryRecord> {
        let history = self.data_cache_parallel_run_history_for_protocol(protocol);
        (!history.is_empty()).then(|| RiscvDataCacheRunHistoryRecord::new(protocol, history))
    }

    pub fn data_cache_parallel_run_history_records(&self) -> Vec<RiscvDataCacheRunHistoryRecord> {
        self.data_cache_parallel_run_histories_by_protocol()
            .into_iter()
            .map(|(protocol, history)| RiscvDataCacheRunHistoryRecord::new(protocol, history))
            .collect()
    }

    pub fn attributed_data_cache_parallel_run_history(&self) -> ParallelCoherenceRunHistory {
        self.data_cache_parallel_run_history()
    }

    pub fn attributed_data_cache_parallel_run_count(&self) -> usize {
        self.attributed_data_cache_parallel_run_history()
            .run_count()
    }

    pub fn unattributed_data_cache_parallel_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::default()
    }

    pub fn unattributed_data_cache_parallel_run_count(&self) -> usize {
        self.unattributed_data_cache_parallel_run_history()
            .run_count()
    }
}
