use super::*;

impl Rem6HostActionSummary {
    pub(crate) const fn checkpoint_restored_count(&self) -> u64 {
        self.checkpoint_restores.len() as u64
    }

    pub(crate) fn checkpoint_restored_component_count(&self) -> u64 {
        self.checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::component_count)
            .sum()
    }

    pub(crate) fn checkpoint_restored_chunk_count(&self) -> u64 {
        self.checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::chunk_count)
            .sum()
    }

    pub(crate) fn checkpoint_restored_payload_bytes(&self) -> u64 {
        self.checkpoint_restores
            .iter()
            .map(Rem6HostCheckpointSummary::payload_bytes)
            .sum()
    }
}

impl Rem6HostCheckpointSummary {
    pub(crate) const fn component_count(&self) -> u64 {
        self.components.len() as u64
    }

    pub(crate) fn chunk_count(&self) -> u64 {
        self.components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::chunk_count)
            .sum()
    }

    pub(crate) fn payload_bytes(&self) -> u64 {
        self.components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::payload_bytes)
            .sum()
    }
}

impl Rem6HostCheckpointComponentSummary {
    pub(crate) const fn chunk_count(&self) -> u64 {
        self.chunks.len() as u64
    }

    pub(crate) fn payload_bytes(&self) -> u64 {
        self.chunks.iter().map(|chunk| chunk.payload_bytes).sum()
    }
}

impl Rem6ExecutionModeStateTransferSummary {
    pub(crate) const fn component_count(&self) -> u64 {
        self.components.len() as u64
    }

    pub(crate) fn chunk_count(&self) -> u64 {
        self.components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::chunk_count)
            .sum()
    }

    pub(crate) fn payload_bytes(&self) -> u64 {
        self.components
            .iter()
            .map(Rem6HostCheckpointComponentSummary::payload_bytes)
            .sum()
    }
}
