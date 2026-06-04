use std::collections::BTreeMap;

use rem6_fabric::{QosPriority, QosQueueArbiter};
use rem6_kernel::WaitForGraph;
use rem6_memory::{
    AccessSize, Address, MemoryError, MemoryOperation, MemoryRequest, MemoryResponse,
    MemoryTargetId, PartitionedMemorySnapshot, PartitionedMemoryStore,
};

use crate::{
    merge_wait_for_graph, profile_snapshot, DramAccess, DramController, DramControllerConfig,
    DramControllerSnapshot, DramMemoryActivityMarker, DramMemoryActivityProfile, DramMemoryError,
    DramQosRequest, DramQosSchedulingPolicy, DramTargetActivity, DramWaitForMarker,
    ExternalMemoryParallelResourceSummary, ExternalMemoryProfile,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryWaitForMarker {
    targets: BTreeMap<MemoryTargetId, DramWaitForMarker>,
}

impl DramMemoryWaitForMarker {
    fn new(targets: BTreeMap<MemoryTargetId, DramWaitForMarker>) -> Self {
        Self { targets }
    }

    fn marker_for(&self, target: MemoryTargetId) -> Option<DramWaitForMarker> {
        self.targets.get(&target).copied()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryOutcome {
    target: MemoryTargetId,
    dram_access: DramAccess,
    response: Option<MemoryResponse>,
}

impl DramMemoryOutcome {
    fn new(
        target: MemoryTargetId,
        dram_access: DramAccess,
        response: Option<MemoryResponse>,
    ) -> Self {
        Self {
            target,
            dram_access,
            response,
        }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn arrival_cycle(&self) -> u64 {
        self.dram_access.arrival_cycle()
    }

    pub const fn ready_cycle(&self) -> u64 {
        self.dram_access.ready_cycle()
    }

    pub const fn dram_access(&self) -> &DramAccess {
        &self.dram_access
    }

    pub fn response(&self) -> Option<&MemoryResponse> {
        self.response.as_ref()
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DramMemoryController {
    store: PartitionedMemoryStore,
    dram: BTreeMap<MemoryTargetId, DramController>,
    profiles: BTreeMap<MemoryTargetId, ExternalMemoryProfile>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemorySnapshot {
    store: PartitionedMemorySnapshot,
    targets: Vec<DramMemoryTargetSnapshot>,
}

impl DramMemorySnapshot {
    pub fn new(store: PartitionedMemorySnapshot, targets: Vec<DramMemoryTargetSnapshot>) -> Self {
        Self { store, targets }
    }

    pub const fn store(&self) -> &PartitionedMemorySnapshot {
        &self.store
    }

    pub fn targets(&self) -> &[DramMemoryTargetSnapshot] {
        &self.targets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryTargetSnapshot {
    target: MemoryTargetId,
    controller: DramControllerSnapshot,
    profile: Option<ExternalMemoryProfile>,
}

impl DramMemoryTargetSnapshot {
    pub const fn new(target: MemoryTargetId, controller: DramControllerSnapshot) -> Self {
        Self {
            target,
            controller,
            profile: None,
        }
    }

    pub const fn with_profile(
        target: MemoryTargetId,
        controller: DramControllerSnapshot,
        profile: ExternalMemoryProfile,
    ) -> Self {
        Self {
            target,
            controller,
            profile: Some(profile),
        }
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn controller(&self) -> &DramControllerSnapshot {
        &self.controller
    }

    pub const fn profile(&self) -> Option<&ExternalMemoryProfile> {
        self.profile.as_ref()
    }
}

impl DramMemoryController {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_target(&mut self, config: DramControllerConfig) -> Result<(), DramMemoryError> {
        if config.layout().bytes() != config.geometry().line_size() {
            return Err(DramMemoryError::TargetLineSizeMismatch {
                target: config.target(),
                layout: config.layout().bytes(),
                geometry: config.geometry().line_size(),
            });
        }

        self.store
            .add_partition(config.target(), config.layout())
            .map_err(DramMemoryError::Memory)?;
        self.dram
            .insert(config.target(), DramController::with_config(config));
        Ok(())
    }

    pub fn add_profile(&mut self, profile: ExternalMemoryProfile) -> Result<(), DramMemoryError> {
        self.add_target(profile.controller_config())?;
        self.profiles.insert(profile.target(), profile);
        Ok(())
    }

    pub fn map_region(
        &mut self,
        target: MemoryTargetId,
        start: Address,
        size: AccessSize,
    ) -> Result<(), DramMemoryError> {
        self.store
            .map_region(target, start, size)
            .map_err(DramMemoryError::Memory)
    }

    pub fn insert_line(
        &mut self,
        target: MemoryTargetId,
        line: Address,
        data: Vec<u8>,
    ) -> Result<(), DramMemoryError> {
        self.store
            .insert_line(target, line, data)
            .map_err(DramMemoryError::Memory)
    }

    pub fn line_data(
        &self,
        target: MemoryTargetId,
        line: Address,
    ) -> Result<Vec<u8>, DramMemoryError> {
        self.store
            .line_data(target, line)
            .map_err(DramMemoryError::Memory)
    }

    pub fn line_count(&self, target: MemoryTargetId) -> Result<usize, DramMemoryError> {
        self.store
            .line_count(target)
            .map_err(DramMemoryError::Memory)
    }

    pub fn target_count(&self) -> usize {
        self.dram.len()
    }

    pub fn dram_controller(&self, target: MemoryTargetId) -> Option<&DramController> {
        self.dram.get(&target)
    }

    pub fn memory_profile(&self, target: MemoryTargetId) -> Option<&ExternalMemoryProfile> {
        self.profiles.get(&target)
    }

    pub fn profile_parallel_resource_summary(
        &self,
        target: MemoryTargetId,
    ) -> Option<ExternalMemoryParallelResourceSummary> {
        self.memory_profile(target)
            .map(|profile| profile.parallel_resource_summary())
    }

    pub fn profile_parallel_resource_summaries(
        &self,
    ) -> Vec<ExternalMemoryParallelResourceSummary> {
        self.profiles
            .values()
            .map(|profile| profile.parallel_resource_summary())
            .collect()
    }

    pub fn mark_activity(&self) -> DramMemoryActivityMarker {
        DramMemoryActivityMarker::new(
            self.dram
                .iter()
                .map(|(target, controller)| (*target, controller.mark_activity()))
                .collect(),
        )
    }

    pub fn mark_wait_for(&self) -> DramMemoryWaitForMarker {
        DramMemoryWaitForMarker::new(
            self.dram
                .iter()
                .map(|(target, controller)| (*target, controller.mark_wait_for()))
                .collect(),
        )
    }

    pub fn target_activity(&self, target: MemoryTargetId) -> Option<DramTargetActivity> {
        self.dram.get(&target).map(|controller| {
            let activity = DramTargetActivity::new(target, controller.activity_profile());
            match self.profiles.get(&target).copied() {
                Some(profile) => activity.with_memory_profile(profile),
                None => activity,
            }
        })
    }

    pub fn target_activity_until(
        &self,
        target: MemoryTargetId,
        end_cycle: u64,
    ) -> Option<DramTargetActivity> {
        self.dram.get(&target).map(|controller| {
            let activity =
                DramTargetActivity::new(target, controller.activity_profile_until(end_cycle));
            match self.profiles.get(&target).copied() {
                Some(profile) => activity.with_memory_profile(profile),
                None => activity,
            }
        })
    }

    pub fn target_activity_since(
        &self,
        marker: &DramMemoryActivityMarker,
        target: MemoryTargetId,
    ) -> Option<DramTargetActivity> {
        self.dram.get(&target).map(|controller| {
            let profile = marker.marker_for(target).map_or_else(
                || controller.activity_profile(),
                |marker| controller.activity_profile_since(marker),
            );
            let activity = DramTargetActivity::new(target, profile);
            match self.profiles.get(&target).copied() {
                Some(profile) => activity.with_memory_profile(profile),
                None => activity,
            }
        })
    }

    pub fn target_activity_since_until(
        &self,
        marker: &DramMemoryActivityMarker,
        target: MemoryTargetId,
        end_cycle: u64,
    ) -> Option<DramTargetActivity> {
        self.dram.get(&target).map(|controller| {
            let profile = marker.marker_for(target).map_or_else(
                || controller.activity_profile_until(end_cycle),
                |marker| controller.activity_profile_since_until(marker, end_cycle),
            );
            let activity = DramTargetActivity::new(target, profile);
            match self.profiles.get(&target).copied() {
                Some(profile) => activity.with_memory_profile(profile),
                None => activity,
            }
        })
    }

    pub fn target_activities(&self) -> Vec<DramTargetActivity> {
        self.dram
            .keys()
            .filter_map(|target| self.target_activity(*target))
            .collect()
    }

    pub fn target_activities_until(&self, end_cycle: u64) -> Vec<DramTargetActivity> {
        self.dram
            .keys()
            .filter_map(|target| self.target_activity_until(*target, end_cycle))
            .filter(|activity| !activity.profile().is_empty())
            .collect()
    }

    pub fn target_activities_since(
        &self,
        marker: &DramMemoryActivityMarker,
    ) -> Vec<DramTargetActivity> {
        self.dram
            .keys()
            .filter_map(|target| self.target_activity_since(marker, *target))
            .filter(|activity| !activity.profile().is_empty())
            .collect()
    }

    pub fn target_activities_since_until(
        &self,
        marker: &DramMemoryActivityMarker,
        end_cycle: u64,
    ) -> Vec<DramTargetActivity> {
        self.dram
            .keys()
            .filter_map(|target| self.target_activity_since_until(marker, *target, end_cycle))
            .filter(|activity| !activity.profile().is_empty())
            .collect()
    }

    pub fn activity_profile(&self) -> DramMemoryActivityProfile {
        DramMemoryActivityProfile::from_target_activities(self.target_activities().iter())
    }

    pub fn activity_profile_until(&self, end_cycle: u64) -> DramMemoryActivityProfile {
        DramMemoryActivityProfile::from_target_activities(
            self.target_activities_until(end_cycle).iter(),
        )
    }

    pub fn activity_profile_since(
        &self,
        marker: &DramMemoryActivityMarker,
    ) -> DramMemoryActivityProfile {
        let activities = self
            .dram
            .keys()
            .filter_map(|target| self.target_activity_since(marker, *target))
            .collect::<Vec<_>>();
        DramMemoryActivityProfile::from_target_activities(activities.iter())
    }

    pub fn activity_profile_since_until(
        &self,
        marker: &DramMemoryActivityMarker,
        end_cycle: u64,
    ) -> DramMemoryActivityProfile {
        let activities = self.target_activities_since_until(marker, end_cycle);
        DramMemoryActivityProfile::from_target_activities(activities.iter())
    }

    pub fn target_wait_for_graph_since(
        &self,
        marker: &DramMemoryWaitForMarker,
        target: MemoryTargetId,
    ) -> Option<WaitForGraph> {
        self.dram.get(&target).map(|controller| {
            let marker = marker
                .marker_for(target)
                .unwrap_or_else(|| DramWaitForMarker::new(0));
            controller.wait_for_graph_since_with_target(marker, Some(target))
        })
    }

    pub fn wait_for_graph_since(&self, marker: &DramMemoryWaitForMarker) -> WaitForGraph {
        let mut graph = WaitForGraph::new();
        for target in self.dram.keys() {
            let Some(target_graph) = self.target_wait_for_graph_since(marker, *target) else {
                continue;
            };
            merge_wait_for_graph(&mut graph, target_graph);
        }
        graph
    }

    pub fn snapshot(&self) -> DramMemorySnapshot {
        DramMemorySnapshot::new(
            self.store.snapshot(),
            self.dram
                .iter()
                .map(|(target, controller)| {
                    if let Some(profile) = self.profiles.get(target).copied() {
                        DramMemoryTargetSnapshot::with_profile(
                            *target,
                            controller.snapshot(),
                            profile,
                        )
                    } else {
                        DramMemoryTargetSnapshot::new(*target, controller.snapshot())
                    }
                })
                .collect(),
        )
    }

    pub fn restore(&mut self, snapshot: &DramMemorySnapshot) -> Result<(), DramMemoryError> {
        *self = Self::from_snapshot(snapshot)?;
        Ok(())
    }

    pub fn from_snapshot(snapshot: &DramMemorySnapshot) -> Result<Self, DramMemoryError> {
        let store = PartitionedMemoryStore::from_snapshot(snapshot.store())
            .map_err(DramMemoryError::Memory)?;
        let mut dram = BTreeMap::new();
        let mut profiles = BTreeMap::new();
        for target in snapshot.targets() {
            if !store.contains_partition(target.target()) {
                return Err(DramMemoryError::Memory(MemoryError::UnknownMemoryTarget {
                    target: target.target(),
                }));
            }
            let partition_layout = store
                .partition_layout(target.target())
                .map_err(DramMemoryError::Memory)?;
            if dram
                .insert(
                    target.target(),
                    DramController::from_snapshot(target.controller()),
                )
                .is_some()
            {
                return Err(DramMemoryError::Memory(
                    MemoryError::DuplicateMemoryTarget {
                        target: target.target(),
                    },
                ));
            }
            if let Some(profile) = target.profile().copied() {
                profile_snapshot::validate_profile_snapshot(
                    target.target(),
                    partition_layout,
                    target.controller(),
                    profile,
                )?;
                profiles.insert(target.target(), profile);
            }
        }
        for partition in store.snapshot().partitions() {
            if !dram.contains_key(&partition.target()) {
                return Err(DramMemoryError::MissingDramTarget {
                    target: partition.target(),
                });
            }
        }

        Ok(Self {
            store,
            dram,
            profiles,
        })
    }
}

impl DramMemoryController {
    pub fn accept(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let target = self
            .store
            .decode_request(request)
            .map_err(DramMemoryError::Memory)?;
        self.preflight_storage(target, request)
            .map_err(DramMemoryError::Memory)?;
        let dram_access = self
            .dram
            .get_mut(&target)
            .expect("DRAM target is inserted with memory target")
            .schedule(arrival_cycle, request)
            .map_err(|source| DramMemoryError::Dram { target, source })?;
        let response = self
            .store
            .respond(request)
            .map_err(DramMemoryError::Memory)?
            .response()
            .cloned();

        Ok(DramMemoryOutcome::new(target, dram_access, response))
    }

    pub fn accept_qos_with_policy(
        &mut self,
        arrival_cycle: u64,
        request: &MemoryRequest,
        priority: QosPriority,
        order: u64,
        arbiter: &mut QosQueueArbiter,
        policy: DramQosSchedulingPolicy,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let mut outcomes = self.accept_qos_batch_with_policy(
            arrival_cycle,
            [DramQosRequest::new(request, priority, order)],
            arbiter,
            policy,
        )?;
        Ok(outcomes
            .pop()
            .expect("single DRAM QoS request returns one outcome"))
    }

    pub fn accept_qos_batch_with_policy<'a, I>(
        &mut self,
        arrival_cycle: u64,
        requests: I,
        arbiter: &mut QosQueueArbiter,
        policy: DramQosSchedulingPolicy,
    ) -> Result<Vec<DramMemoryOutcome>, DramMemoryError>
    where
        I: IntoIterator<Item = DramQosRequest<'a>>,
    {
        let requests = requests.into_iter().collect::<Vec<_>>();
        if requests.is_empty() {
            return Ok(Vec::new());
        }

        let mut by_target = BTreeMap::<MemoryTargetId, Vec<DramQosRequest<'a>>>::new();
        for request in requests {
            let target = self
                .store
                .decode_request(request.request())
                .map_err(DramMemoryError::Memory)?;
            self.preflight_storage(target, request.request())
                .map_err(DramMemoryError::Memory)?;
            if !self.dram.contains_key(&target) {
                return Err(DramMemoryError::MissingDramTarget { target });
            }
            by_target.entry(target).or_default().push(request);
        }

        let mut outcomes = Vec::new();
        for (target, requests) in by_target {
            let request_by_id = requests
                .iter()
                .map(|request| (request.request().id(), request.request()))
                .collect::<BTreeMap<_, _>>();
            let accesses = self
                .dram
                .get_mut(&target)
                .expect("DRAM target is inserted with memory target")
                .schedule_qos_batch_with_policy(arrival_cycle, requests, arbiter, policy)
                .map_err(|source| DramMemoryError::Dram { target, source })?;
            for access in accesses {
                let request = request_by_id
                    .get(&access.request())
                    .expect("DRAM access comes from the accepted batch request");
                let response = self
                    .store
                    .respond(request)
                    .map_err(DramMemoryError::Memory)?
                    .response()
                    .cloned();
                outcomes.push(DramMemoryOutcome::new(target, access, response));
            }
        }

        Ok(outcomes)
    }

    fn preflight_storage(
        &self,
        target: MemoryTargetId,
        request: &MemoryRequest,
    ) -> Result<(), MemoryError> {
        if request.line_span() != 1 {
            return Err(MemoryError::CrossLineAccess {
                request: request.id(),
                start: request.range().start(),
                size: request.size(),
                line_size: request.line_layout().bytes(),
            });
        }

        if matches!(
            request.operation(),
            MemoryOperation::WriteClean
                | MemoryOperation::WritebackClean
                | MemoryOperation::WritebackDirty
        ) {
            return Ok(());
        }

        self.store
            .line_data(target, request.line_address())
            .map(|_| ())
    }
}
