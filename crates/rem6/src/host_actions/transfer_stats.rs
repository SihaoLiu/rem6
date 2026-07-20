use std::collections::{BTreeMap, BTreeSet};

use super::{
    Rem6ExecutionModeStateTransferSummary, Rem6HostCheckpointChunkSummary,
    Rem6HostCheckpointComponentSummary, Rem6HostCheckpointSummary,
    Rem6HostO3RuntimeCheckpointStatValue,
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostActionTransferStats {
    pub(crate) components: u64,
    pub(crate) chunks: u64,
    pub(crate) payload_bytes: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostActionChunkStats {
    pub(crate) chunks: u64,
    pub(crate) payload_bytes: u64,
    pub(crate) payload_checksum_accumulator: u64,
    pub(crate) o3_runtime_numeric: BTreeMap<String, Rem6HostO3RuntimeCheckpointStatValue>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostActionComponentStats {
    pub(crate) components: BTreeMap<String, HostActionTransferStats>,
    pub(crate) chunks: BTreeMap<(String, String), HostActionChunkStats>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct HostActionTargetStats {
    pub(crate) transfers: BTreeMap<String, HostActionTransferStats>,
    pub(crate) components: BTreeMap<(String, String), HostActionTransferStats>,
    pub(crate) chunks: BTreeMap<(String, String, String), HostActionChunkStats>,
}

impl HostActionComponentStats {
    pub(crate) fn from_components<'a>(
        components: impl IntoIterator<Item = &'a Rem6HostCheckpointComponentSummary>,
        stat_path_segment: &impl Fn(&str) -> String,
    ) -> Self {
        let mut stats = Self::default();
        for component in components {
            stats.add_component(component, stat_path_segment);
        }
        stats
    }

    fn add_component(
        &mut self,
        component: &Rem6HostCheckpointComponentSummary,
        stat_path_segment: &impl Fn(&str) -> String,
    ) {
        let component_path = stat_path_segment(&component.component);
        add_transfer(
            self.components.entry(component_path.clone()).or_default(),
            1,
            component.chunk_count(),
            component.payload_bytes(),
        );
        for chunk in &component.chunks {
            let chunk_path = stat_path_segment(&chunk.name);
            add_chunk(
                self.chunks
                    .entry((component_path.clone(), chunk_path))
                    .or_default(),
                chunk,
            );
        }
    }
}

impl HostActionTargetStats {
    pub(crate) fn add_restore_targets(
        &mut self,
        restore: &Rem6HostCheckpointSummary,
        stat_path_segment: &impl Fn(&str) -> String,
    ) {
        let target_paths = restore
            .execution_modes
            .iter()
            .map(|authority| stat_path_segment(&authority.target))
            .collect::<BTreeSet<_>>();
        for component in &restore.components {
            let component_path = stat_path_segment(&component.component);
            if !target_paths.contains(&component_path) {
                continue;
            }
            add_transfer(
                self.transfers.entry(component_path.clone()).or_default(),
                1,
                component.chunk_count(),
                component.payload_bytes(),
            );
            self.add_component(
                component_path.clone(),
                component_path,
                component,
                stat_path_segment,
            );
        }
    }

    pub(crate) fn add_switch_transfer(
        &mut self,
        target: String,
        transfer: &Rem6ExecutionModeStateTransferSummary,
        stat_path_segment: &impl Fn(&str) -> String,
    ) {
        add_transfer(
            self.transfers.entry(target.clone()).or_default(),
            transfer.component_count(),
            transfer.chunk_count(),
            transfer.payload_bytes(),
        );
        for component in &transfer.components {
            self.add_component(
                target.clone(),
                stat_path_segment(&component.component),
                component,
                stat_path_segment,
            );
        }
    }

    fn add_component(
        &mut self,
        target: String,
        component_path: String,
        component: &Rem6HostCheckpointComponentSummary,
        stat_path_segment: &impl Fn(&str) -> String,
    ) {
        add_transfer(
            self.components
                .entry((target.clone(), component_path.clone()))
                .or_default(),
            1,
            component.chunk_count(),
            component.payload_bytes(),
        );
        for chunk in &component.chunks {
            let chunk_path = stat_path_segment(&chunk.name);
            add_chunk(
                self.chunks
                    .entry((target.clone(), component_path.clone(), chunk_path))
                    .or_default(),
                chunk,
            );
        }
    }
}

fn add_transfer(
    stats: &mut HostActionTransferStats,
    components: u64,
    chunks: u64,
    payload_bytes: u64,
) {
    stats.components += components;
    stats.chunks += chunks;
    stats.payload_bytes += payload_bytes;
}

fn add_chunk(stats: &mut HostActionChunkStats, chunk: &Rem6HostCheckpointChunkSummary) {
    stats.chunks += 1;
    stats.payload_bytes += chunk.payload_bytes;
    stats.payload_checksum_accumulator = stats
        .payload_checksum_accumulator
        .wrapping_add(chunk.payload_checksum);
    let Some(o3_runtime) = &chunk.o3_runtime else {
        return;
    };
    for (field, value) in o3_runtime.numeric_stat_fields() {
        stats
            .o3_runtime_numeric
            .entry(field.to_string())
            .and_modify(|current| current.merge_restore_value(value))
            .or_insert(value);
    }
}
