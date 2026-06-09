use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, RiscvCluster, RiscvDataAccessEvent, RiscvDataAccessEventKind};
use rem6_memory::MemoryOperation;
use rem6_stats::{
    MemProbePacket, MemProbePacketAccess, ProbePayload, ProbePointId, ProbeRegistry, ProbeSnapshot,
    StackDistProbe, StackDistProbeConfig, StackDistProbeSnapshot, StatsError,
};

use crate::{RiscvSystemRun, RiscvSystemRunDriver, SystemError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataAccessProbeSnapshot {
    probes: ProbeSnapshot,
    stack_distance: StackDistProbeSnapshot,
    request_point: ProbePointId,
}

impl RiscvDataAccessProbeSnapshot {
    pub fn new(
        probes: ProbeSnapshot,
        stack_distance: StackDistProbeSnapshot,
        request_point: ProbePointId,
    ) -> Self {
        Self {
            probes,
            stack_distance,
            request_point,
        }
    }

    pub const fn probes(&self) -> &ProbeSnapshot {
        &self.probes
    }

    pub const fn stack_distance(&self) -> &StackDistProbeSnapshot {
        &self.stack_distance
    }

    pub const fn request_point(&self) -> ProbePointId {
        self.request_point
    }
}

#[derive(Debug)]
pub struct RiscvDataAccessStats {
    probes: Arc<Mutex<RiscvDataAccessProbeRecorder>>,
}

impl Clone for RiscvDataAccessStats {
    fn clone(&self) -> Self {
        let config = self
            .probes
            .lock()
            .expect("data access probe recorder lock")
            .stack_distance_config()
            .clone();
        Self {
            probes: Arc::new(Mutex::new(RiscvDataAccessProbeRecorder::new(config))),
        }
    }
}

impl RiscvDataAccessStats {
    pub fn with_stack_distance(config: StackDistProbeConfig) -> Self {
        Self {
            probes: Arc::new(Mutex::new(RiscvDataAccessProbeRecorder::new(config))),
        }
    }

    pub(crate) fn reset_for_run<I>(&self, cursors: I)
    where
        I: IntoIterator<Item = (CpuId, usize)>,
    {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .reset(cursors);
    }

    pub(crate) fn record_data_access_events<I>(&self, events_by_cpu: I) -> Result<(), StatsError>
    where
        I: IntoIterator<Item = (CpuId, Vec<RiscvDataAccessEvent>)>,
    {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .record_data_access_events(events_by_cpu)
    }

    pub fn data_access_probe_snapshot(&self) -> RiscvDataAccessProbeSnapshot {
        self.probes
            .lock()
            .expect("data access probe recorder lock")
            .snapshot()
    }
}

impl RiscvSystemRun {
    pub fn with_data_access_probes(
        mut self,
        data_access_probes: Option<RiscvDataAccessProbeSnapshot>,
    ) -> Self {
        self.data_access_probes = data_access_probes;
        self
    }

    pub const fn data_access_probes(&self) -> Option<&RiscvDataAccessProbeSnapshot> {
        self.data_access_probes.as_ref()
    }
}

impl RiscvSystemRunDriver {
    pub fn with_data_access_stats(mut self, data_access_stats: RiscvDataAccessStats) -> Self {
        self.data_access_stats = Some(data_access_stats);
        self
    }

    pub const fn data_access_stats(&self) -> Option<&RiscvDataAccessStats> {
        self.data_access_stats.as_ref()
    }

    pub(crate) fn reset_data_access_stats_for_run(
        &self,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        if let Some(data_access_stats) = &self.data_access_stats {
            data_access_stats.reset_for_run(data_access_event_cursors(cluster)?);
        }
        Ok(())
    }

    pub(crate) fn record_data_access_stats(
        &self,
        cluster: &RiscvCluster,
    ) -> Result<(), SystemError> {
        let Some(data_access_stats) = &self.data_access_stats else {
            return Ok(());
        };
        data_access_stats
            .record_data_access_events(data_access_event_snapshots(cluster)?)
            .map_err(SystemError::Stats)
    }
}

fn data_access_event_cursors(cluster: &RiscvCluster) -> Result<Vec<(CpuId, usize)>, SystemError> {
    data_access_event_snapshots(cluster).map(|events| {
        events
            .into_iter()
            .map(|(cpu, events)| (cpu, events.len()))
            .collect()
    })
}

fn data_access_event_snapshots(
    cluster: &RiscvCluster,
) -> Result<Vec<(CpuId, Vec<RiscvDataAccessEvent>)>, SystemError> {
    cluster
        .core_ids()
        .into_iter()
        .map(|cpu| {
            cluster
                .core(cpu)
                .map(|core| (cpu, core.data_access_events()))
                .map_err(SystemError::RiscvCluster)
        })
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvDataAccessProbeRecorder {
    stack_distance_config: StackDistProbeConfig,
    probes: ProbeRegistry,
    stack_distance: StackDistProbe,
    request_point: ProbePointId,
    cursors: BTreeMap<CpuId, usize>,
}

impl RiscvDataAccessProbeRecorder {
    fn new(stack_distance_config: StackDistProbeConfig) -> Self {
        let mut recorder = Self {
            stack_distance_config: stack_distance_config.clone(),
            probes: ProbeRegistry::new(),
            stack_distance: StackDistProbe::new(stack_distance_config),
            request_point: ProbePointId::new(0),
            cursors: BTreeMap::new(),
        };
        recorder.reset([]);
        recorder
    }

    fn reset<I>(&mut self, cursors: I)
    where
        I: IntoIterator<Item = (CpuId, usize)>,
    {
        self.probes = ProbeRegistry::new();
        self.stack_distance = StackDistProbe::new(self.stack_distance_config.clone());
        self.request_point = self
            .probes
            .register_point("riscv_data", "Request")
            .expect("generated data access probe point is valid");
        self.probes
            .add_listener(self.request_point, "stack_dist")
            .expect("generated data access probe listener is valid");
        self.cursors = cursors.into_iter().collect();
    }

    fn record_data_access_events<I>(&mut self, events_by_cpu: I) -> Result<(), StatsError>
    where
        I: IntoIterator<Item = (CpuId, Vec<RiscvDataAccessEvent>)>,
    {
        let mut new_events = Vec::new();
        let mut new_cursors = Vec::new();
        for (cpu, events) in events_by_cpu {
            let cursor = self.cursors.get(&cpu).copied().unwrap_or(0);
            for event in events.iter().skip(cursor) {
                new_events.push((cpu, event.clone()));
            }
            new_cursors.push((cpu, events.len()));
        }

        new_events.sort_by_key(|(cpu, event)| (event.tick(), *cpu));
        for (_cpu, event) in new_events {
            self.record_event(&event)?;
        }
        for (cpu, cursor) in new_cursors {
            self.cursors.insert(cpu, cursor);
        }
        Ok(())
    }

    fn record_event(&mut self, event: &RiscvDataAccessEvent) -> Result<(), StatsError> {
        if event.kind() != RiscvDataAccessEventKind::Issued || event.route().is_none() {
            return Ok(());
        }
        let Some(access) = packet_access(event.operation()) else {
            return Ok(());
        };
        let packet = MemProbePacket::request(event.physical_address().get())
            .with_access(access)
            .with_size(event.size().bytes())
            .with_packet_id(packet_id(event));
        let event = self
            .probes
            .emit(
                event.tick(),
                self.request_point,
                ProbePayload::MemoryPacket(packet),
            )?
            .clone();
        self.stack_distance
            .observe_probe_event(&event, self.request_point)?;
        Ok(())
    }

    fn snapshot(&self) -> RiscvDataAccessProbeSnapshot {
        RiscvDataAccessProbeSnapshot::new(
            self.probes.snapshot(),
            self.stack_distance.snapshot(),
            self.request_point,
        )
    }

    fn stack_distance_config(&self) -> &StackDistProbeConfig {
        &self.stack_distance_config
    }
}

fn packet_id(event: &RiscvDataAccessEvent) -> u64 {
    (u64::from(event.request_id().agent().get()) << 32)
        | (event.request_id().sequence() & u64::from(u32::MAX))
}

fn packet_access(operation: MemoryOperation) -> Option<MemProbePacketAccess> {
    match operation {
        MemoryOperation::ReadShared | MemoryOperation::ReadUnique | MemoryOperation::LoadLocked => {
            Some(MemProbePacketAccess::Read)
        }
        MemoryOperation::Write
        | MemoryOperation::StoreConditional
        | MemoryOperation::StoreConditionalFail
        | MemoryOperation::LockedRmwWrite
        | MemoryOperation::Atomic
        | MemoryOperation::AtomicNoReturn => Some(MemProbePacketAccess::Write),
        _ => None,
    }
}
