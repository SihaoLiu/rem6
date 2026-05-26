use std::collections::BTreeMap;

use rem6_kernel::WaitForEdgeKind;

use super::{WorkloadParallelExecutionSummary, WorkloadWaitForEdgeKindWindow};

impl WorkloadParallelExecutionSummary {
    pub fn with_data_cache_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.data_cache_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.data_cache_wait_for_edge_count =
            self.data_cache_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.data_cache_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_data_cache_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.data_cache_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        self.data_cache_wait_for_edge_kind_counts =
            wait_for_edge_kind_counts_from_windows(&self.data_cache_wait_for_edge_kind_windows);
        self.data_cache_wait_for_edge_count =
            self.data_cache_wait_for_edge_count
                .max(wait_for_edge_kind_window_count_sum(
                    &self.data_cache_wait_for_edge_kind_windows,
                ));
        self
    }

    pub fn with_resource_wait_for_edge_kind_counts(
        mut self,
        fabric_counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
        dram_counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.fabric_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(fabric_counts);
        self.dram_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(dram_counts);
        self.fabric_wait_for_edge_count =
            self.fabric_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.fabric_wait_for_edge_kind_counts,
                ));
        self.dram_wait_for_edge_count =
            self.dram_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.dram_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_resource_wait_for_edge_kind_windows(
        mut self,
        fabric_windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
        dram_windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.fabric_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(fabric_windows);
        self.dram_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(dram_windows);
        self.fabric_wait_for_edge_kind_counts =
            wait_for_edge_kind_counts_from_windows(&self.fabric_wait_for_edge_kind_windows);
        self.dram_wait_for_edge_kind_counts =
            wait_for_edge_kind_counts_from_windows(&self.dram_wait_for_edge_kind_windows);
        self.fabric_wait_for_edge_count =
            self.fabric_wait_for_edge_count
                .max(wait_for_edge_kind_window_count_sum(
                    &self.fabric_wait_for_edge_kind_windows,
                ));
        self.dram_wait_for_edge_count =
            self.dram_wait_for_edge_count
                .max(wait_for_edge_kind_window_count_sum(
                    &self.dram_wait_for_edge_kind_windows,
                ));
        self
    }

    pub fn with_gpu_compute_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.gpu_compute_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.gpu_compute_wait_for_edge_count =
            self.gpu_compute_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.gpu_compute_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_gpu_compute_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.gpu_compute_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        self.gpu_compute_wait_for_edge_kind_counts =
            wait_for_edge_kind_counts_from_windows(&self.gpu_compute_wait_for_edge_kind_windows);
        self.gpu_compute_wait_for_edge_count =
            self.gpu_compute_wait_for_edge_count
                .max(wait_for_edge_kind_window_count_sum(
                    &self.gpu_compute_wait_for_edge_kind_windows,
                ));
        self
    }

    pub fn with_gpu_dma_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.gpu_dma_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.gpu_dma_wait_for_edge_count =
            self.gpu_dma_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.gpu_dma_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_gpu_dma_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.gpu_dma_wait_for_edge_kind_windows = collect_wait_for_edge_kind_windows(windows);
        self.gpu_dma_wait_for_edge_kind_counts =
            wait_for_edge_kind_counts_from_windows(&self.gpu_dma_wait_for_edge_kind_windows);
        self.gpu_dma_wait_for_edge_count =
            self.gpu_dma_wait_for_edge_count
                .max(wait_for_edge_kind_window_count_sum(
                    &self.gpu_dma_wait_for_edge_kind_windows,
                ));
        self
    }

    pub fn with_accelerator_compute_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.accelerator_compute_wait_for_edge_kind_counts =
            collect_wait_for_edge_kind_counts(counts);
        self.accelerator_compute_wait_for_edge_count = self
            .accelerator_compute_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.accelerator_compute_wait_for_edge_kind_counts,
            ));
        self
    }

    pub fn with_accelerator_compute_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.accelerator_compute_wait_for_edge_kind_windows =
            collect_wait_for_edge_kind_windows(windows);
        self.accelerator_compute_wait_for_edge_kind_counts = wait_for_edge_kind_counts_from_windows(
            &self.accelerator_compute_wait_for_edge_kind_windows,
        );
        self.accelerator_compute_wait_for_edge_count = self
            .accelerator_compute_wait_for_edge_count
            .max(wait_for_edge_kind_window_count_sum(
                &self.accelerator_compute_wait_for_edge_kind_windows,
            ));
        self
    }

    pub fn with_accelerator_dma_wait_for_edge_kind_counts(
        mut self,
        counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
    ) -> Self {
        self.accelerator_dma_wait_for_edge_kind_counts = collect_wait_for_edge_kind_counts(counts);
        self.accelerator_dma_wait_for_edge_count =
            self.accelerator_dma_wait_for_edge_count
                .max(wait_for_edge_kind_count_sum(
                    &self.accelerator_dma_wait_for_edge_kind_counts,
                ));
        self
    }

    pub fn with_accelerator_dma_wait_for_edge_kind_windows(
        mut self,
        windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
    ) -> Self {
        self.accelerator_dma_wait_for_edge_kind_windows =
            collect_wait_for_edge_kind_windows(windows);
        self.accelerator_dma_wait_for_edge_kind_counts = wait_for_edge_kind_counts_from_windows(
            &self.accelerator_dma_wait_for_edge_kind_windows,
        );
        self.accelerator_dma_wait_for_edge_count =
            self.accelerator_dma_wait_for_edge_count
                .max(wait_for_edge_kind_window_count_sum(
                    &self.accelerator_dma_wait_for_edge_kind_windows,
                ));
        self
    }

    pub fn data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.data_cache_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.data_cache_wait_for_edge_kind_windows,
            ))
    }

    pub fn data_cache_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.data_cache_wait_for_edge_kind_counts
    }

    pub fn data_cache_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.data_cache_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.data_cache_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn data_cache_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.data_cache_wait_for_edge_kind_windows
    }

    pub fn data_cache_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.data_cache_wait_for_edge_kind_windows, kind)
    }

    pub fn fabric_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.fabric_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.fabric_wait_for_edge_kind_windows,
            ))
    }

    pub fn fabric_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.fabric_wait_for_edge_kind_counts
    }

    pub fn fabric_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.fabric_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.fabric_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn fabric_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.fabric_wait_for_edge_kind_windows
    }

    pub fn fabric_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.fabric_wait_for_edge_kind_windows, kind)
    }

    pub fn dram_wait_for_edge_count(&self) -> usize {
        self.dram_wait_for_edge_count
            .max(wait_for_edge_kind_count_sum(
                &self.dram_wait_for_edge_kind_counts,
            ))
            .max(wait_for_edge_kind_window_count_sum(
                &self.dram_wait_for_edge_kind_windows,
            ))
    }

    pub fn dram_wait_for_edge_kind_counts(&self) -> &BTreeMap<WaitForEdgeKind, usize> {
        &self.dram_wait_for_edge_kind_counts
    }

    pub fn dram_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_kind_count(&self.dram_wait_for_edge_kind_counts, kind).max(
            wait_for_edge_kind_window(&self.dram_wait_for_edge_kind_windows, kind)
                .map(|window| window.edge_count())
                .unwrap_or(0),
        )
    }

    pub fn dram_wait_for_edge_kind_windows(&self) -> &[WorkloadWaitForEdgeKindWindow] {
        &self.dram_wait_for_edge_kind_windows
    }

    pub fn dram_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.dram_wait_for_edge_kind_windows, kind)
    }

    pub fn resource_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count() + self.dram_wait_for_edge_count()
    }

    pub fn resource_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        merge_wait_for_edge_kind_counts([
            self.fabric_wait_for_edge_kind_counts(),
            self.dram_wait_for_edge_kind_counts(),
        ])
    }

    pub fn resource_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.fabric_wait_for_edge_count_by_kind(kind) + self.dram_wait_for_edge_count_by_kind(kind)
    }

    pub fn resource_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        merge_wait_for_edge_kind_windows(
            self.fabric_wait_for_edge_kind_windows
                .iter()
                .copied()
                .chain(self.dram_wait_for_edge_kind_windows.iter().copied()),
        )
    }

    pub fn resource_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.resource_wait_for_edge_kind_windows(), kind)
    }

    pub fn resource_activity_count(&self) -> usize {
        self.fabric_transfer_count + self.dram_access_count + self.resource_wait_for_edge_count()
    }

    pub fn has_resource_activity(&self) -> bool {
        self.resource_activity_count() != 0
    }

    pub fn full_system_wait_for_edge_count(&self) -> usize {
        self.resource_wait_for_edge_count()
            + self.data_cache_wait_for_edge_count()
            + self.compute_wait_for_edge_count()
            + self.dma_wait_for_edge_count()
    }

    pub fn full_system_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        merge_wait_for_edge_kind_counts([
            &self.resource_wait_for_edge_kind_counts(),
            self.data_cache_wait_for_edge_kind_counts(),
            &self.compute_wait_for_edge_kind_counts(),
            &self.dma_wait_for_edge_kind_counts(),
        ])
    }

    pub fn full_system_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.resource_wait_for_edge_count_by_kind(kind)
            + self.data_cache_wait_for_edge_count_by_kind(kind)
            + self.compute_wait_for_edge_count_by_kind(kind)
            + self.dma_wait_for_edge_count_by_kind(kind)
    }

    pub fn full_system_wait_for_edge_kind_windows(&self) -> Vec<WorkloadWaitForEdgeKindWindow> {
        let resource_windows = self.resource_wait_for_edge_kind_windows();
        let compute_windows = self.compute_wait_for_edge_kind_windows();
        let dma_windows = self.dma_wait_for_edge_kind_windows();
        merge_wait_for_edge_kind_windows(
            resource_windows
                .into_iter()
                .chain(self.data_cache_wait_for_edge_kind_windows.iter().copied())
                .chain(compute_windows)
                .chain(dma_windows),
        )
    }

    pub fn full_system_wait_for_edge_kind_window(
        &self,
        kind: WaitForEdgeKind,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        wait_for_edge_kind_window(&self.full_system_wait_for_edge_kind_windows(), kind)
    }
}

pub(super) fn collect_wait_for_edge_kind_counts(
    counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut by_kind = BTreeMap::new();
    for (kind, count) in counts {
        if count == 0 {
            continue;
        }
        let stored = by_kind.entry(kind).or_insert(0usize);
        *stored = stored.saturating_add(count);
    }
    by_kind
}

pub(super) fn wait_for_edge_kind_count(
    counts: &BTreeMap<WaitForEdgeKind, usize>,
    kind: WaitForEdgeKind,
) -> usize {
    counts.get(&kind).copied().unwrap_or(0)
}

pub(super) fn wait_for_edge_kind_count_sum(counts: &BTreeMap<WaitForEdgeKind, usize>) -> usize {
    counts.values().copied().sum()
}

pub(super) fn merge_wait_for_edge_kind_counts<'a>(
    maps: impl IntoIterator<Item = &'a BTreeMap<WaitForEdgeKind, usize>>,
) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut merged = BTreeMap::new();
    for map in maps {
        for (kind, count) in map {
            let stored = merged.entry(*kind).or_insert(0usize);
            *stored = stored.saturating_add(*count);
        }
    }
    merged
}

pub(super) fn collect_wait_for_edge_kind_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    let mut by_kind = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_kind
            .entry(window.kind())
            .and_modify(|stored: &mut WorkloadWaitForEdgeKindWindow| stored.merge(window))
            .or_insert(window);
    }
    by_kind.into_values().collect()
}

pub(super) fn merge_wait_for_edge_kind_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    collect_wait_for_edge_kind_windows(windows)
}

pub(super) fn wait_for_edge_kind_window(
    windows: &[WorkloadWaitForEdgeKindWindow],
    kind: WaitForEdgeKind,
) -> Option<WorkloadWaitForEdgeKindWindow> {
    windows.iter().copied().find(|window| window.kind() == kind)
}

pub(super) fn wait_for_edge_kind_window_count_sum(
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> usize {
    windows
        .iter()
        .map(WorkloadWaitForEdgeKindWindow::edge_count)
        .sum()
}

pub(super) fn wait_for_edge_kind_counts_from_windows(
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> BTreeMap<WaitForEdgeKind, usize> {
    collect_wait_for_edge_kind_counts(
        windows
            .iter()
            .map(|window| (window.kind(), window.edge_count())),
    )
}
