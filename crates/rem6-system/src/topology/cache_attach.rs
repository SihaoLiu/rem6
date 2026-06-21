use std::sync::{Arc, Mutex};

use rem6_coherence::{
    MsiBankDirectoryHarness, ParallelCoherenceRunSummary, PartitionedChiDirectoryLineHarness,
    PartitionedDirectoryLineHarness, PartitionedMesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarness,
};

use super::coherence_data::{
    RiscvTopologyChiDataCache, RiscvTopologyMesiDataCache, RiscvTopologyMoesiDataCache,
    RiscvTopologyMsiBankDataCache, RiscvTopologyMsiDataCache,
};
use super::instruction_cache::RiscvTopologyMsiInstructionCache;
use super::{RiscvTopologySystem, RiscvTopologySystemError};

impl RiscvTopologySystem {
    pub fn with_msi_instruction_cache(
        mut self,
        harness: PartitionedDirectoryLineHarness,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.msi_instruction_cache = Some(RiscvTopologyMsiInstructionCache::new(harness));
        Ok(self)
    }

    pub fn with_mesi_data_cache(
        mut self,
        harness: PartitionedMesiDirectoryLineHarness,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.mesi_data_cache = Some(RiscvTopologyMesiDataCache::new(harness));
        Ok(self)
    }

    pub fn with_msi_data_cache(
        mut self,
        harness: PartitionedDirectoryLineHarness,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.msi_data_cache = Some(RiscvTopologyMsiDataCache::new(harness));
        Ok(self)
    }

    pub fn with_msi_bank_data_cache(
        mut self,
        harness: MsiBankDirectoryHarness,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.msi_bank_data_cache = Some(RiscvTopologyMsiBankDataCache::new(harness));
        Ok(self)
    }

    pub fn with_moesi_data_cache(
        mut self,
        harness: PartitionedMoesiDirectoryLineHarness,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.moesi_data_cache = Some(RiscvTopologyMoesiDataCache::new(harness));
        Ok(self)
    }

    pub fn with_chi_data_cache(
        mut self,
        harness: PartitionedChiDirectoryLineHarness,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.chi_data_cache = Some(RiscvTopologyChiDataCache::new(harness));
        Ok(self)
    }

    pub fn msi_instruction_cache(&self) -> Option<Arc<Mutex<PartitionedDirectoryLineHarness>>> {
        self.msi_instruction_cache
            .as_ref()
            .map(RiscvTopologyMsiInstructionCache::harness)
    }

    pub fn msi_instruction_cache_runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.msi_instruction_cache
            .as_ref()
            .map(RiscvTopologyMsiInstructionCache::runs)
            .unwrap_or_default()
    }

    pub fn msi_data_cache(&self) -> Option<Arc<Mutex<PartitionedDirectoryLineHarness>>> {
        self.msi_data_cache
            .as_ref()
            .map(RiscvTopologyMsiDataCache::harness)
    }

    pub fn msi_bank_data_cache(&self) -> Option<Arc<Mutex<MsiBankDirectoryHarness>>> {
        self.msi_bank_data_cache
            .as_ref()
            .map(RiscvTopologyMsiBankDataCache::harness)
    }

    pub fn msi_bank_data_cache_runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.msi_bank_data_cache
            .as_ref()
            .map(RiscvTopologyMsiBankDataCache::runs)
            .unwrap_or_default()
    }

    pub fn msi_data_cache_runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.msi_data_cache
            .as_ref()
            .map(RiscvTopologyMsiDataCache::runs)
            .unwrap_or_default()
    }

    pub fn mesi_data_cache(&self) -> Option<Arc<Mutex<PartitionedMesiDirectoryLineHarness>>> {
        self.mesi_data_cache
            .as_ref()
            .map(RiscvTopologyMesiDataCache::harness)
    }

    pub fn mesi_data_cache_runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.mesi_data_cache
            .as_ref()
            .map(RiscvTopologyMesiDataCache::runs)
            .unwrap_or_default()
    }

    pub fn moesi_data_cache(&self) -> Option<Arc<Mutex<PartitionedMoesiDirectoryLineHarness>>> {
        self.moesi_data_cache
            .as_ref()
            .map(RiscvTopologyMoesiDataCache::harness)
    }

    pub fn moesi_data_cache_runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.moesi_data_cache
            .as_ref()
            .map(RiscvTopologyMoesiDataCache::runs)
            .unwrap_or_default()
    }

    pub fn chi_data_cache(&self) -> Option<Arc<Mutex<PartitionedChiDirectoryLineHarness>>> {
        self.chi_data_cache
            .as_ref()
            .map(RiscvTopologyChiDataCache::harness)
    }

    pub fn chi_data_cache_runs(&self) -> Vec<ParallelCoherenceRunSummary> {
        self.chi_data_cache
            .as_ref()
            .map(RiscvTopologyChiDataCache::runs)
            .unwrap_or_default()
    }
}
