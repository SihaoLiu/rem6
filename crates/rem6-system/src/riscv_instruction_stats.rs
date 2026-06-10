use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cpu::CpuId;
use rem6_kernel::Tick;
use rem6_stats::{
    GlobalInstTracker, GlobalInstTrackerSnapshot, LocalInstTracker, PcCountPair, PcCountTracker,
    PcCountTrackerManager, PcCountTrackerSnapshot, ProbePayload, ProbePointId, ProbeRegistry,
    ProbeSnapshot, StatId, StatsError,
};

use crate::RiscvSystemRun;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvRetiredInstructionProbeSnapshot {
    probes: ProbeSnapshot,
    tracker: GlobalInstTrackerSnapshot,
    pc_count: Option<PcCountTrackerSnapshot>,
    points: BTreeMap<CpuId, ProbePointId>,
    pc_points: BTreeMap<CpuId, ProbePointId>,
}

impl RiscvRetiredInstructionProbeSnapshot {
    pub fn new(
        probes: ProbeSnapshot,
        tracker: GlobalInstTrackerSnapshot,
        pc_count: Option<PcCountTrackerSnapshot>,
        points: BTreeMap<CpuId, ProbePointId>,
        pc_points: BTreeMap<CpuId, ProbePointId>,
    ) -> Self {
        Self {
            probes,
            tracker,
            pc_count,
            points,
            pc_points,
        }
    }

    pub const fn probes(&self) -> &ProbeSnapshot {
        &self.probes
    }

    pub const fn tracker(&self) -> &GlobalInstTrackerSnapshot {
        &self.tracker
    }

    pub const fn pc_count(&self) -> Option<&PcCountTrackerSnapshot> {
        self.pc_count.as_ref()
    }

    pub fn points(&self) -> &BTreeMap<CpuId, ProbePointId> {
        &self.points
    }

    pub fn point_for_cpu(&self, cpu: CpuId) -> Option<ProbePointId> {
        self.points.get(&cpu).copied()
    }

    pub fn retired_pc_points(&self) -> &BTreeMap<CpuId, ProbePointId> {
        &self.pc_points
    }

    pub fn retired_pc_point_for_cpu(&self, cpu: CpuId) -> Option<ProbePointId> {
        self.pc_points.get(&cpu).copied()
    }
}

impl RiscvSystemRun {
    pub fn with_retired_instruction_probes(
        mut self,
        retired_instruction_probes: Option<RiscvRetiredInstructionProbeSnapshot>,
    ) -> Self {
        self.retired_instruction_probes = retired_instruction_probes;
        self
    }

    pub const fn retired_instruction_probes(
        &self,
    ) -> Option<&RiscvRetiredInstructionProbeSnapshot> {
        self.retired_instruction_probes.as_ref()
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
        let pc_targets = self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .pc_targets()
            .to_vec();
        Self {
            committed,
            retired_instruction_probes: Arc::new(Mutex::new(
                RiscvRetiredInstructionProbeRecorder::new(cpus, thresholds, pc_targets),
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
                RiscvRetiredInstructionProbeRecorder::new(cpus, Vec::new(), Vec::new()),
            )),
        }
    }

    pub fn with_retired_inst_thresholds<I>(self, thresholds: I) -> Self
    where
        I: IntoIterator<Item = u64>,
    {
        let cpus = self.committed.keys().copied().collect::<Vec<_>>();
        let pc_targets = self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .pc_targets()
            .to_vec();
        *self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock") =
            RiscvRetiredInstructionProbeRecorder::new(cpus, thresholds, pc_targets);
        self
    }

    pub fn with_pc_count_targets<I>(self, targets: I) -> Self
    where
        I: IntoIterator<Item = PcCountPair>,
    {
        let cpus = self.committed.keys().copied().collect::<Vec<_>>();
        let thresholds = self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .thresholds()
            .to_vec();
        *self
            .retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock") =
            RiscvRetiredInstructionProbeRecorder::new(cpus, thresholds, targets);
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
        pc: u64,
    ) -> Result<(), StatsError> {
        self.retired_instruction_probes
            .lock()
            .expect("retired instruction probe recorder lock")
            .record(cpu, tick, pc)
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
    pc_targets: Vec<PcCountPair>,
    probes: ProbeRegistry,
    global: GlobalInstTracker,
    local: BTreeMap<CpuId, LocalInstTracker>,
    pc_tracker: Option<PcCountTracker>,
    pc_manager: Option<PcCountTrackerManager>,
    points: BTreeMap<CpuId, ProbePointId>,
    pc_points: BTreeMap<CpuId, ProbePointId>,
}

impl RiscvRetiredInstructionProbeRecorder {
    fn new<I, T, P>(cpus: I, thresholds: T, pc_targets: P) -> Self
    where
        I: IntoIterator<Item = CpuId>,
        T: IntoIterator<Item = u64>,
        P: IntoIterator<Item = PcCountPair>,
    {
        let thresholds = thresholds.into_iter().collect::<Vec<_>>();
        let pc_targets = pc_targets.into_iter().collect::<Vec<_>>();
        let mut recorder = Self {
            thresholds,
            pc_targets,
            probes: ProbeRegistry::new(),
            global: GlobalInstTracker::new(Vec::new()),
            local: BTreeMap::new(),
            pc_tracker: None,
            pc_manager: None,
            points: BTreeMap::new(),
            pc_points: BTreeMap::new(),
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
        self.pc_tracker = if self.pc_targets.is_empty() {
            None
        } else {
            Some(PcCountTracker::new(self.pc_targets.clone()))
        };
        self.pc_manager = if self.pc_targets.is_empty() {
            None
        } else {
            Some(PcCountTrackerManager::new(self.pc_targets.clone()))
        };
        self.local.clear();
        self.points.clear();
        self.pc_points.clear();
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
            if self.pc_tracker.is_some() {
                let pc_point = self
                    .probes
                    .register_point(format!("riscv_cpu_{}", cpu.get()), "RetiredPC")
                    .expect("generated retired PC probe point is valid");
                self.probes
                    .add_listener(pc_point, "pc_count_tracker")
                    .expect("generated retired PC probe listener is valid");
                self.pc_points.insert(cpu, pc_point);
            }
        }
    }

    fn record(&mut self, cpu: CpuId, tick: Tick, pc: u64) -> Result<(), StatsError> {
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
        if let (Some(pc_point), Some(tracker), Some(manager)) = (
            self.pc_points.get(&cpu).copied(),
            self.pc_tracker.as_ref(),
            self.pc_manager.as_mut(),
        ) {
            let event = self
                .probes
                .emit(tick, pc_point, ProbePayload::ProgramCounter { pc })?
                .clone();
            tracker.observe_retired_pc_probe_event(&event, pc_point, manager);
        }
        Ok(())
    }

    fn snapshot(&self) -> RiscvRetiredInstructionProbeSnapshot {
        RiscvRetiredInstructionProbeSnapshot::new(
            self.probes.snapshot(),
            self.global.snapshot(),
            self.pc_manager
                .as_ref()
                .map(PcCountTrackerManager::snapshot),
            self.points.clone(),
            self.pc_points.clone(),
        )
    }

    fn thresholds(&self) -> &[u64] {
        &self.thresholds
    }

    fn pc_targets(&self) -> &[PcCountPair] {
        &self.pc_targets
    }
}
