use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::CpuId;
use rem6_kernel::Tick;
use rem6_stats::{
    GlobalInstTracker, GlobalInstTrackerSnapshot, LocalInstTracker, ProbePayload, ProbePointId,
    ProbeRegistry, ProbeSnapshot, StatId, StatsError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvRetiredInstructionProbeSnapshot {
    probes: ProbeSnapshot,
    tracker: GlobalInstTrackerSnapshot,
    points: BTreeMap<CpuId, ProbePointId>,
}

impl RiscvRetiredInstructionProbeSnapshot {
    pub fn new(
        probes: ProbeSnapshot,
        tracker: GlobalInstTrackerSnapshot,
        points: BTreeMap<CpuId, ProbePointId>,
    ) -> Self {
        Self {
            probes,
            tracker,
            points,
        }
    }

    pub const fn probes(&self) -> &ProbeSnapshot {
        &self.probes
    }

    pub const fn tracker(&self) -> &GlobalInstTrackerSnapshot {
        &self.tracker
    }

    pub fn points(&self) -> &BTreeMap<CpuId, ProbePointId> {
        &self.points
    }

    pub fn point_for_cpu(&self, cpu: CpuId) -> Option<ProbePointId> {
        self.points.get(&cpu).copied()
    }
}

#[derive(Debug)]
pub struct RiscvInstructionStats {
    committed: BTreeMap<CpuId, StatId>,
    retired_instruction_probes: Arc<Mutex<RiscvRetiredInstructionProbeRecorder>>,
}

impl Clone for RiscvInstructionStats {
    fn clone(&self) -> Self {
        let committed = self.committed.clone();
        let cpus = committed.keys().copied().collect::<Vec<_>>();
        let thresholds = self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .thresholds()
            .to_vec();
        Self {
            committed,
            retired_instruction_probes: Arc::new(Mutex::new(
                RiscvRetiredInstructionProbeRecorder::new(cpus, thresholds),
            )),
        }
    }
}

impl RiscvInstructionStats {
    pub fn new<I>(committed: I) -> Self
    where
        I: IntoIterator<Item = (CpuId, StatId)>,
    {
        let committed = committed.into_iter().collect::<BTreeMap<_, _>>();
        let cpus = committed.keys().copied().collect::<Vec<_>>();
        Self {
            committed,
            retired_instruction_probes: Arc::new(Mutex::new(
                RiscvRetiredInstructionProbeRecorder::new(cpus, Vec::new()),
            )),
        }
    }

    pub fn with_retired_inst_thresholds<I>(self, thresholds: I) -> Self
    where
        I: IntoIterator<Item = u64>,
    {
        let cpus = self.committed.keys().copied().collect::<Vec<_>>();
        *self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock") =
            RiscvRetiredInstructionProbeRecorder::new(cpus, thresholds);
        self
    }

    pub fn committed_stat(&self, cpu: CpuId) -> Option<StatId> {
        self.committed.get(&cpu).copied()
    }

    pub fn committed_stats(&self) -> &BTreeMap<CpuId, StatId> {
        &self.committed
    }

    pub fn reset_retired_instruction_probes(&self) {
        let cpus = self.committed.keys().copied().collect::<Vec<_>>();
        self.retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .reset(cpus);
    }

    pub fn record_retired_instruction_probe(
        &self,
        cpu: CpuId,
        tick: Tick,
    ) -> Result<(), StatsError> {
        self.retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .record(cpu, tick)
    }

    pub fn retired_instruction_probe_snapshot(&self) -> RiscvRetiredInstructionProbeSnapshot {
        self.retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .snapshot()
    }
}

impl Default for RiscvInstructionStats {
    fn default() -> Self {
        Self::new([])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvRetiredInstructionProbeRecorder {
    thresholds: Vec<u64>,
    probes: ProbeRegistry,
    global: GlobalInstTracker,
    local: BTreeMap<CpuId, LocalInstTracker>,
    points: BTreeMap<CpuId, ProbePointId>,
}

impl RiscvRetiredInstructionProbeRecorder {
    fn new<I, T>(cpus: I, thresholds: T) -> Self
    where
        I: IntoIterator<Item = CpuId>,
        T: IntoIterator<Item = u64>,
    {
        let thresholds = thresholds.into_iter().collect::<Vec<_>>();
        let mut recorder = Self {
            thresholds,
            probes: ProbeRegistry::new(),
            global: GlobalInstTracker::new(Vec::new()),
            local: BTreeMap::new(),
            points: BTreeMap::new(),
        };
        recorder.reset(cpus);
        recorder
    }

    fn reset<I>(&mut self, cpus: I)
    where
        I: IntoIterator<Item = CpuId>,
    {
        self.probes = ProbeRegistry::new();
        self.global = GlobalInstTracker::new(self.thresholds.clone());
        self.local.clear();
        self.points.clear();
        for cpu in cpus {
            let point = self
                .probes
                .register_point(format!("riscv_cpu_{}", cpu.get()), "RetiredInsts")
                .expect("generated retired instruction probe point is valid");
            self.probes
                .add_listener(point, "inst_tracker")
                .expect("generated retired instruction probe listener is valid");
            self.local.insert(cpu, LocalInstTracker::new(true));
            self.points.insert(cpu, point);
        }
    }

    fn record(&mut self, cpu: CpuId, tick: Tick) -> Result<(), StatsError> {
        let Some(point) = self.points.get(&cpu).copied() else {
            return Ok(());
        };
        let event = self
            .probes
            .emit(tick, point, ProbePayload::Counter { amount: 1 })?
            .clone();
        if let Some(local) = self.local.get(&cpu) {
            local.observe_retired_insts_probe_event(&event, point, &mut self.global)?;
        }
        Ok(())
    }

    fn snapshot(&self) -> RiscvRetiredInstructionProbeSnapshot {
        RiscvRetiredInstructionProbeSnapshot::new(
            self.probes.snapshot(),
            self.global.snapshot(),
            self.points.clone(),
        )
    }

    fn thresholds(&self) -> &[u64] {
        &self.thresholds
    }
}
