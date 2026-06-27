use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_cpu::{
    BiModeBranchPredictorCheckpointPayload, BiModeBranchPredictorError,
    BranchPredictorCheckpointPayload, BranchPredictorError, GShareBranchPredictorCheckpointPayload,
    GShareBranchPredictorError, InOrderPipelineCheckpointPayload, InOrderPipelineError,
    InOrderPipelineSnapshot, MultiperspectivePerceptronCheckpointPayload,
    MultiperspectivePerceptronError, O3PendingStateCheckpointPayload, O3PipelineError, RiscvCore,
    RiscvHartRunState, TageScLBranchPredictorCheckpointPayload, TageScLBranchPredictorError,
    TournamentBranchPredictorCheckpointPayload, TournamentBranchPredictorError,
};
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvPmpConfig, RiscvPmpError, RiscvPmpSnapshot,
    RiscvPmpSnapshotEntry, RiscvPmpTable,
};
use rem6_memory::Address;

const FREGS_CHUNK: &str = "fregs";
const BIMODE_BRANCH_PREDICTOR_CHUNK: &str = "bimode-branch-predictor";
const BRANCH_PREDICTOR_CHUNK: &str = "branch-predictor";
const GSHARE_BRANCH_PREDICTOR_CHUNK: &str = "gshare-branch-predictor";
const HART_RUN_STATE_CHUNK: &str = "hart-run-state";
const IN_ORDER_PIPELINE_CHUNK: &str = "in-order-pipeline";
const MULTIPERSPECTIVE_PERCEPTRON_CHUNK: &str = "multiperspective-perceptron";
const O3_PENDING_STATE_CHUNK: &str = "o3-pending-state";
const PC_CHUNK: &str = "pc";
const TAGE_SC_L_BRANCH_PREDICTOR_CHUNK: &str = "tage-sc-l-branch-predictor";
const TOURNAMENT_BRANCH_PREDICTOR_CHUNK: &str = "tournament-branch-predictor";
const XREGS_CHUNK: &str = "xregs";
const PMP_CHUNK: &str = "pmp";
const U64_BYTES: usize = 8;
const FREG_COUNT: usize = 32;
const FREG_BYTES: usize = FREG_COUNT * U64_BYTES;
const XREG_COUNT: usize = 32;
const XREG_BYTES: usize = XREG_COUNT * U64_BYTES;
const PMP_HEADER_BYTES: usize = 2;
const PMP_ENTRY_BYTES: usize = U64_BYTES + 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCoreCheckpointRecord {
    component: CheckpointComponentId,
    pc: Address,
    registers: Vec<(Register, u64)>,
    float_registers: Vec<(FloatRegister, u64)>,
    pmp_snapshot: RiscvPmpSnapshot,
    hart_run_state: RiscvHartRunState,
    in_order_pipeline_snapshot: InOrderPipelineSnapshot,
    branch_predictor_payload: BranchPredictorCheckpointPayload,
    gshare_branch_predictor_payload: GShareBranchPredictorCheckpointPayload,
    bimode_branch_predictor_payload: BiModeBranchPredictorCheckpointPayload,
    tournament_branch_predictor_payload: TournamentBranchPredictorCheckpointPayload,
    tage_sc_l_branch_predictor_payload: TageScLBranchPredictorCheckpointPayload,
    multiperspective_perceptron_payload: MultiperspectivePerceptronCheckpointPayload,
    o3_pending_state_payload: O3PendingStateCheckpointPayload,
}

struct RiscvCoreCheckpointRecordParts {
    component: CheckpointComponentId,
    pc: Address,
    registers: Vec<(Register, u64)>,
    float_registers: Vec<(FloatRegister, u64)>,
    pmp_snapshot: RiscvPmpSnapshot,
    hart_run_state: RiscvHartRunState,
    in_order_pipeline_snapshot: InOrderPipelineSnapshot,
    branch_predictor_payload: BranchPredictorCheckpointPayload,
    gshare_branch_predictor_payload: GShareBranchPredictorCheckpointPayload,
    bimode_branch_predictor_payload: BiModeBranchPredictorCheckpointPayload,
    tournament_branch_predictor_payload: TournamentBranchPredictorCheckpointPayload,
    tage_sc_l_branch_predictor_payload: TageScLBranchPredictorCheckpointPayload,
    multiperspective_perceptron_payload: MultiperspectivePerceptronCheckpointPayload,
    o3_pending_state_payload: O3PendingStateCheckpointPayload,
}

impl RiscvCoreCheckpointRecord {
    pub fn new(
        component: CheckpointComponentId,
        pc: Address,
        registers: Vec<(Register, u64)>,
        pmp_snapshot: RiscvPmpSnapshot,
    ) -> Self {
        Self::from_parts(RiscvCoreCheckpointRecordParts {
            component,
            pc,
            registers,
            float_registers: zero_float_register_values(),
            pmp_snapshot,
            hart_run_state: RiscvHartRunState::Started,
            in_order_pipeline_snapshot: RiscvCore::default_in_order_pipeline_snapshot(),
            branch_predictor_payload: RiscvCore::default_branch_predictor_checkpoint_payload(),
            gshare_branch_predictor_payload:
                RiscvCore::default_gshare_branch_predictor_checkpoint_payload(),
            bimode_branch_predictor_payload:
                RiscvCore::default_bimode_branch_predictor_checkpoint_payload(),
            tournament_branch_predictor_payload:
                RiscvCore::default_tournament_branch_predictor_checkpoint_payload(),
            tage_sc_l_branch_predictor_payload:
                RiscvCore::default_tage_sc_l_branch_predictor_checkpoint_payload(),
            multiperspective_perceptron_payload:
                RiscvCore::default_multiperspective_perceptron_checkpoint_payload(),
            o3_pending_state_payload: RiscvCore::default_o3_pending_state_checkpoint_payload(),
        })
    }

    fn from_parts(parts: RiscvCoreCheckpointRecordParts) -> Self {
        Self {
            component: parts.component,
            pc: parts.pc,
            registers: parts.registers,
            float_registers: parts.float_registers,
            pmp_snapshot: parts.pmp_snapshot,
            hart_run_state: parts.hart_run_state,
            in_order_pipeline_snapshot: parts.in_order_pipeline_snapshot,
            branch_predictor_payload: parts.branch_predictor_payload,
            gshare_branch_predictor_payload: parts.gshare_branch_predictor_payload,
            bimode_branch_predictor_payload: parts.bimode_branch_predictor_payload,
            tournament_branch_predictor_payload: parts.tournament_branch_predictor_payload,
            tage_sc_l_branch_predictor_payload: parts.tage_sc_l_branch_predictor_payload,
            multiperspective_perceptron_payload: parts.multiperspective_perceptron_payload,
            o3_pending_state_payload: parts.o3_pending_state_payload,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub fn registers(&self) -> &[(Register, u64)] {
        &self.registers
    }

    pub fn float_registers(&self) -> &[(FloatRegister, u64)] {
        &self.float_registers
    }

    pub fn pmp_snapshot(&self) -> &RiscvPmpSnapshot {
        &self.pmp_snapshot
    }

    pub const fn hart_run_state(&self) -> RiscvHartRunState {
        self.hart_run_state
    }

    pub const fn in_order_pipeline_snapshot(&self) -> &InOrderPipelineSnapshot {
        &self.in_order_pipeline_snapshot
    }

    pub const fn branch_predictor_payload(&self) -> &BranchPredictorCheckpointPayload {
        &self.branch_predictor_payload
    }

    pub const fn gshare_branch_predictor_payload(&self) -> &GShareBranchPredictorCheckpointPayload {
        &self.gshare_branch_predictor_payload
    }

    pub const fn bimode_branch_predictor_payload(&self) -> &BiModeBranchPredictorCheckpointPayload {
        &self.bimode_branch_predictor_payload
    }

    pub const fn tournament_branch_predictor_payload(
        &self,
    ) -> &TournamentBranchPredictorCheckpointPayload {
        &self.tournament_branch_predictor_payload
    }

    pub const fn tage_sc_l_branch_predictor_payload(
        &self,
    ) -> &TageScLBranchPredictorCheckpointPayload {
        &self.tage_sc_l_branch_predictor_payload
    }

    pub const fn multiperspective_perceptron_payload(
        &self,
    ) -> &MultiperspectivePerceptronCheckpointPayload {
        &self.multiperspective_perceptron_payload
    }

    pub const fn o3_pending_state_payload(&self) -> &O3PendingStateCheckpointPayload {
        &self.o3_pending_state_payload
    }

    pub fn register(&self, register: Register) -> Option<u64> {
        self.registers
            .iter()
            .find_map(|(current, value)| (*current == register).then_some(*value))
    }

    pub fn float_register(&self, register: FloatRegister) -> Option<u64> {
        self.float_registers
            .iter()
            .find_map(|(current, value)| (*current == register).then_some(*value))
    }
}

#[derive(Clone, Debug)]
pub struct RiscvCoreCheckpointPort {
    component: CheckpointComponentId,
    core: RiscvCore,
}

impl RiscvCoreCheckpointPort {
    pub const fn new(component: CheckpointComponentId, core: RiscvCore) -> Self {
        Self { component, core }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn core(&self) -> RiscvCore {
        self.core.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<RiscvCoreCheckpointRecord, CheckpointError> {
        let record = self.capture_record();
        registry.write_chunk(
            &self.component,
            PC_CHUNK,
            record.pc().get().to_le_bytes().to_vec(),
        )?;
        registry.write_chunk(
            &self.component,
            XREGS_CHUNK,
            encode_registers(record.registers()),
        )?;
        registry.write_chunk(
            &self.component,
            FREGS_CHUNK,
            encode_float_registers(record.float_registers()),
        )?;
        registry.write_chunk(
            &self.component,
            HART_RUN_STATE_CHUNK,
            encode_hart_run_state(record.hart_run_state()),
        )?;
        registry.write_chunk(
            &self.component,
            PMP_CHUNK,
            encode_pmp_snapshot(record.pmp_snapshot()),
        )?;
        registry.write_chunk(
            &self.component,
            IN_ORDER_PIPELINE_CHUNK,
            encode_in_order_pipeline_snapshot(record.in_order_pipeline_snapshot()),
        )?;
        registry.write_chunk(
            &self.component,
            BRANCH_PREDICTOR_CHUNK,
            encode_branch_predictor_payload(record.branch_predictor_payload()),
        )?;
        registry.write_chunk(
            &self.component,
            GSHARE_BRANCH_PREDICTOR_CHUNK,
            encode_gshare_branch_predictor_payload(record.gshare_branch_predictor_payload()),
        )?;
        registry.write_chunk(
            &self.component,
            BIMODE_BRANCH_PREDICTOR_CHUNK,
            encode_bimode_branch_predictor_payload(record.bimode_branch_predictor_payload()),
        )?;
        registry.write_chunk(
            &self.component,
            TOURNAMENT_BRANCH_PREDICTOR_CHUNK,
            encode_tournament_branch_predictor_payload(
                record.tournament_branch_predictor_payload(),
            ),
        )?;
        registry.write_chunk(
            &self.component,
            TAGE_SC_L_BRANCH_PREDICTOR_CHUNK,
            encode_tage_sc_l_branch_predictor_payload(record.tage_sc_l_branch_predictor_payload()),
        )?;
        registry.write_chunk(
            &self.component,
            MULTIPERSPECTIVE_PERCEPTRON_CHUNK,
            encode_multiperspective_perceptron_payload(
                record.multiperspective_perceptron_payload(),
            ),
        )?;
        registry.write_chunk(
            &self.component,
            O3_PENDING_STATE_CHUNK,
            encode_o3_pending_state_payload(record.o3_pending_state_payload()),
        )?;
        Ok(record)
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<RiscvCoreCheckpointRecord, RiscvCoreCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_record(&record)?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<RiscvCoreCheckpointRecord, RiscvCoreCheckpointError> {
        let pc = decode_pc(
            &self.component,
            registry.chunk(&self.component, PC_CHUNK).ok_or_else(|| {
                RiscvCoreCheckpointError::MissingChunk {
                    component: self.component.clone(),
                    name: PC_CHUNK.to_string(),
                }
            })?,
        )?;
        let registers = decode_registers(
            &self.component,
            registry
                .chunk(&self.component, XREGS_CHUNK)
                .ok_or_else(|| RiscvCoreCheckpointError::MissingChunk {
                    component: self.component.clone(),
                    name: XREGS_CHUNK.to_string(),
                })?,
        )?;
        let float_registers = match registry.chunk(&self.component, FREGS_CHUNK) {
            Some(payload) => decode_float_registers(&self.component, payload)?,
            None => zero_float_register_values(),
        };
        let pmp_snapshot = decode_pmp_snapshot(
            &self.component,
            registry.chunk(&self.component, PMP_CHUNK).ok_or_else(|| {
                RiscvCoreCheckpointError::MissingChunk {
                    component: self.component.clone(),
                    name: PMP_CHUNK.to_string(),
                }
            })?,
            self.core.pmp_entry_count(),
        )?;
        let hart_run_state = match registry.chunk(&self.component, HART_RUN_STATE_CHUNK) {
            Some(payload) => decode_hart_run_state(&self.component, payload)?,
            None => RiscvHartRunState::Started,
        };
        let in_order_pipeline_snapshot =
            match registry.chunk(&self.component, IN_ORDER_PIPELINE_CHUNK) {
                Some(payload) => decode_in_order_pipeline_snapshot(&self.component, payload)?,
                None => RiscvCore::default_in_order_pipeline_snapshot(),
            };
        let branch_predictor_payload = match registry.chunk(&self.component, BRANCH_PREDICTOR_CHUNK)
        {
            Some(payload) => decode_branch_predictor_payload(&self.component, payload)?,
            None => RiscvCore::default_branch_predictor_checkpoint_payload(),
        };
        self.core
            .validate_branch_predictor_checkpoint_payload(&branch_predictor_payload)
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        let gshare_branch_predictor_payload =
            match registry.chunk(&self.component, GSHARE_BRANCH_PREDICTOR_CHUNK) {
                Some(payload) => decode_gshare_branch_predictor_payload(&self.component, payload)?,
                None => RiscvCore::default_gshare_branch_predictor_checkpoint_payload(),
            };
        self.core
            .validate_gshare_branch_predictor_checkpoint_payload(&gshare_branch_predictor_payload)
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidGShareBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        let bimode_branch_predictor_payload =
            match registry.chunk(&self.component, BIMODE_BRANCH_PREDICTOR_CHUNK) {
                Some(payload) => decode_bimode_branch_predictor_payload(&self.component, payload)?,
                None => RiscvCore::default_bimode_branch_predictor_checkpoint_payload(),
            };
        self.core
            .validate_bimode_branch_predictor_checkpoint_payload(&bimode_branch_predictor_payload)
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidBiModeBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        let tournament_branch_predictor_payload = match registry
            .chunk(&self.component, TOURNAMENT_BRANCH_PREDICTOR_CHUNK)
        {
            Some(payload) => decode_tournament_branch_predictor_payload(&self.component, payload)?,
            None => RiscvCore::default_tournament_branch_predictor_checkpoint_payload(),
        };
        self.core
            .validate_tournament_branch_predictor_checkpoint_payload(
                &tournament_branch_predictor_payload,
            )
            .map_err(|error| {
                RiscvCoreCheckpointError::InvalidTournamentBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                }
            })?;
        let tage_sc_l_branch_predictor_payload = match registry
            .chunk(&self.component, TAGE_SC_L_BRANCH_PREDICTOR_CHUNK)
        {
            Some(payload) => decode_tage_sc_l_branch_predictor_payload(&self.component, payload)?,
            None => RiscvCore::default_tage_sc_l_branch_predictor_checkpoint_payload(),
        };
        self.core
            .validate_tage_sc_l_branch_predictor_checkpoint_payload(
                &tage_sc_l_branch_predictor_payload,
            )
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidTageScLBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        let multiperspective_perceptron_payload = match registry
            .chunk(&self.component, MULTIPERSPECTIVE_PERCEPTRON_CHUNK)
        {
            Some(payload) => decode_multiperspective_perceptron_payload(&self.component, payload)?,
            None => RiscvCore::default_multiperspective_perceptron_checkpoint_payload(),
        };
        self.core
            .validate_multiperspective_perceptron_checkpoint_payload(
                &multiperspective_perceptron_payload,
            )
            .map_err(|error| {
                RiscvCoreCheckpointError::InvalidMultiperspectivePerceptronSnapshot {
                    component: self.component.clone(),
                    error,
                }
            })?;
        let o3_pending_state_payload = match registry.chunk(&self.component, O3_PENDING_STATE_CHUNK)
        {
            Some(payload) => decode_o3_pending_state_payload(&self.component, payload)?,
            None => RiscvCore::default_o3_pending_state_checkpoint_payload(),
        };
        self.core
            .validate_o3_pending_state_checkpoint_payload(&o3_pending_state_payload)
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidO3PendingStateSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;

        Ok(RiscvCoreCheckpointRecord::from_parts(
            RiscvCoreCheckpointRecordParts {
                component: self.component.clone(),
                pc,
                registers,
                float_registers,
                pmp_snapshot,
                hart_run_state,
                in_order_pipeline_snapshot,
                branch_predictor_payload,
                gshare_branch_predictor_payload,
                bimode_branch_predictor_payload,
                tournament_branch_predictor_payload,
                tage_sc_l_branch_predictor_payload,
                multiperspective_perceptron_payload,
                o3_pending_state_payload,
            },
        ))
    }

    fn restore_record(
        &self,
        record: &RiscvCoreCheckpointRecord,
    ) -> Result<(), RiscvCoreCheckpointError> {
        self.core
            .restore_pmp_snapshot(record.pmp_snapshot())
            .map_err(|error| RiscvCoreCheckpointError::InvalidPmpSnapshot {
                component: self.component.clone(),
                error,
            })?;
        self.core.redirect_pc(record.pc());
        for (register, value) in record.registers() {
            self.core.write_register(*register, *value);
        }
        for (register, value) in record.float_registers() {
            self.core.write_float_register(*register, *value);
        }
        match record.hart_run_state() {
            RiscvHartRunState::Started => self.core.set_hart_started(),
            RiscvHartRunState::StartPending => self.core.set_hart_start_pending(),
            RiscvHartRunState::StopPending => self.core.set_hart_stop_pending(),
            RiscvHartRunState::SuspendPending => self.core.set_hart_suspend_pending(),
            RiscvHartRunState::ResumePending => self.core.set_hart_resume_pending(),
            RiscvHartRunState::Stopped => self.core.set_hart_stopped(),
            RiscvHartRunState::Suspended => self.core.set_hart_suspended(),
        }
        self.core
            .restore_in_order_pipeline_snapshot(record.in_order_pipeline_snapshot().clone())
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidInOrderPipelineSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        self.core
            .restore_branch_predictor_checkpoint_payload(record.branch_predictor_payload().clone())
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        self.core
            .restore_gshare_branch_predictor_checkpoint_payload(
                record.gshare_branch_predictor_payload().clone(),
            )
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidGShareBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        self.core
            .restore_bimode_branch_predictor_checkpoint_payload(
                record.bimode_branch_predictor_payload().clone(),
            )
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidBiModeBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        self.core
            .restore_tournament_branch_predictor_checkpoint_payload(
                record.tournament_branch_predictor_payload().clone(),
            )
            .map_err(|error| {
                RiscvCoreCheckpointError::InvalidTournamentBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                }
            })?;
        self.core
            .restore_tage_sc_l_branch_predictor_checkpoint_payload(
                record.tage_sc_l_branch_predictor_payload().clone(),
            )
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidTageScLBranchPredictorSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        self.core
            .restore_multiperspective_perceptron_checkpoint_payload(
                record.multiperspective_perceptron_payload().clone(),
            )
            .map_err(|error| {
                RiscvCoreCheckpointError::InvalidMultiperspectivePerceptronSnapshot {
                    component: self.component.clone(),
                    error,
                }
            })?;
        self.core
            .restore_o3_pending_state_checkpoint_payload(record.o3_pending_state_payload().clone())
            .map_err(
                |error| RiscvCoreCheckpointError::InvalidO3PendingStateSnapshot {
                    component: self.component.clone(),
                    error,
                },
            )?;
        Ok(())
    }

    fn capture_record(&self) -> RiscvCoreCheckpointRecord {
        RiscvCoreCheckpointRecord::from_parts(RiscvCoreCheckpointRecordParts {
            component: self.component.clone(),
            pc: self.core.pc(),
            registers: all_register_values(&self.core),
            float_registers: all_float_register_values(&self.core),
            pmp_snapshot: self.core.pmp_snapshot(),
            hart_run_state: self.core.hart_run_state(),
            in_order_pipeline_snapshot: self.core.in_order_pipeline_snapshot(),
            branch_predictor_payload: self.core.branch_predictor_checkpoint_payload(),
            gshare_branch_predictor_payload: self.core.gshare_branch_predictor_checkpoint_payload(),
            bimode_branch_predictor_payload: self.core.bimode_branch_predictor_checkpoint_payload(),
            tournament_branch_predictor_payload: self
                .core
                .tournament_branch_predictor_checkpoint_payload(),
            tage_sc_l_branch_predictor_payload: self
                .core
                .tage_sc_l_branch_predictor_checkpoint_payload(),
            multiperspective_perceptron_payload: self
                .core
                .multiperspective_perceptron_checkpoint_payload(),
            o3_pending_state_payload: self.core.o3_pending_state_checkpoint_payload(),
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct RiscvCoreCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, RiscvCoreCheckpointPort>,
}

impl RiscvCoreCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = RiscvCoreCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, RiscvCoreCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            decoded.push((port, port.decode_from(registry)?));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_record(&record)?;
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), RiscvCoreCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunkSize {
        component: CheckpointComponentId,
        name: String,
        expected: usize,
        actual: usize,
    },
    InvalidPmpEntryCount {
        component: CheckpointComponentId,
        expected: usize,
        actual: usize,
    },
    InvalidPmpSnapshot {
        component: CheckpointComponentId,
        error: RiscvPmpError,
    },
    InvalidHartRunState {
        component: CheckpointComponentId,
        value: u8,
    },
    InvalidInOrderPipelineSnapshot {
        component: CheckpointComponentId,
        error: InOrderPipelineError,
    },
    InvalidBranchPredictorSnapshot {
        component: CheckpointComponentId,
        error: BranchPredictorError,
    },
    InvalidGShareBranchPredictorSnapshot {
        component: CheckpointComponentId,
        error: GShareBranchPredictorError,
    },
    InvalidBiModeBranchPredictorSnapshot {
        component: CheckpointComponentId,
        error: BiModeBranchPredictorError,
    },
    InvalidTournamentBranchPredictorSnapshot {
        component: CheckpointComponentId,
        error: TournamentBranchPredictorError,
    },
    InvalidTageScLBranchPredictorSnapshot {
        component: CheckpointComponentId,
        error: TageScLBranchPredictorError,
    },
    InvalidMultiperspectivePerceptronSnapshot {
        component: CheckpointComponentId,
        error: MultiperspectivePerceptronError,
    },
    InvalidO3PendingStateSnapshot {
        component: CheckpointComponentId,
        error: O3PipelineError,
    },
}

impl fmt::Display for RiscvCoreCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "RISC-V core checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunkSize {
                component,
                name,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core checkpoint component {} chunk {name} has {actual} bytes; \
                 expected {expected}",
                component.as_str()
            ),
            Self::InvalidPmpEntryCount {
                component,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core checkpoint component {} has {actual} PMP entries; expected {expected}",
                component.as_str()
            ),
            Self::InvalidPmpSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid PMP snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidHartRunState { component, value } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid hart run-state value {value}",
                component.as_str()
            ),
            Self::InvalidInOrderPipelineSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid in-order pipeline snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidBranchPredictorSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid branch predictor snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidGShareBranchPredictorSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid gshare branch predictor snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidBiModeBranchPredictorSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid bimode branch predictor snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidTournamentBranchPredictorSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid tournament branch predictor snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidTageScLBranchPredictorSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid TAGE-SC-L branch predictor snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidMultiperspectivePerceptronSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid multiperspective perceptron snapshot: {error}",
                component.as_str()
            ),
            Self::InvalidO3PendingStateSnapshot { component, error } => write!(
                formatter,
                "RISC-V core checkpoint component {} has invalid O3 pending-state snapshot: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for RiscvCoreCheckpointError {}

fn all_register_values(core: &RiscvCore) -> Vec<(Register, u64)> {
    (0..XREG_COUNT)
        .map(|index| {
            let register = Register::new(index as u8).expect("register index is valid");
            (register, core.read_register(register))
        })
        .collect()
}

fn all_float_register_values(core: &RiscvCore) -> Vec<(FloatRegister, u64)> {
    (0..FREG_COUNT)
        .map(|index| {
            let register = FloatRegister::new(index as u8).expect("register index is valid");
            (register, core.read_float_register(register))
        })
        .collect()
}

fn zero_float_register_values() -> Vec<(FloatRegister, u64)> {
    (0..FREG_COUNT)
        .map(|index| {
            let register = FloatRegister::new(index as u8).expect("register index is valid");
            (register, 0)
        })
        .collect()
}

fn encode_registers(registers: &[(Register, u64)]) -> Vec<u8> {
    let mut payload = vec![0; XREG_BYTES];
    for (register, value) in registers {
        let offset = usize::from(register.index()) * U64_BYTES;
        payload[offset..offset + U64_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    payload
}

fn encode_float_registers(registers: &[(FloatRegister, u64)]) -> Vec<u8> {
    let mut payload = vec![0; FREG_BYTES];
    for (register, value) in registers {
        let offset = usize::from(register.index()) * U64_BYTES;
        payload[offset..offset + U64_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    payload
}

fn encode_hart_run_state(state: RiscvHartRunState) -> Vec<u8> {
    vec![match state {
        RiscvHartRunState::Started => 0,
        RiscvHartRunState::Stopped => 1,
        RiscvHartRunState::Suspended => 2,
        RiscvHartRunState::StartPending => 3,
        RiscvHartRunState::StopPending => 4,
        RiscvHartRunState::SuspendPending => 5,
        RiscvHartRunState::ResumePending => 6,
    }]
}

fn decode_hart_run_state(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<RiscvHartRunState, RiscvCoreCheckpointError> {
    if payload.len() != 1 {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: HART_RUN_STATE_CHUNK.to_string(),
            expected: 1,
            actual: payload.len(),
        });
    }
    match payload[0] {
        0 => Ok(RiscvHartRunState::Started),
        1 => Ok(RiscvHartRunState::Stopped),
        2 => Ok(RiscvHartRunState::Suspended),
        3 => Ok(RiscvHartRunState::StartPending),
        4 => Ok(RiscvHartRunState::StopPending),
        5 => Ok(RiscvHartRunState::SuspendPending),
        6 => Ok(RiscvHartRunState::ResumePending),
        value => Err(RiscvCoreCheckpointError::InvalidHartRunState {
            component: component.clone(),
            value,
        }),
    }
}

fn encode_pmp_snapshot(snapshot: &RiscvPmpSnapshot) -> Vec<u8> {
    let entry_count = snapshot.entries().len();
    debug_assert!(u16::try_from(entry_count).is_ok());
    let mut payload = Vec::with_capacity(PMP_HEADER_BYTES + entry_count * PMP_ENTRY_BYTES);
    payload.extend_from_slice(&(entry_count as u16).to_le_bytes());
    for entry in snapshot.entries() {
        payload.extend_from_slice(&entry.raw_addr().to_le_bytes());
        payload.push(entry.config().bits());
    }
    payload
}

fn encode_in_order_pipeline_snapshot(snapshot: &InOrderPipelineSnapshot) -> Vec<u8> {
    InOrderPipelineCheckpointPayload::from_snapshot(snapshot.clone())
        .expect("captured RISC-V core in-order pipeline snapshot is valid")
        .encode()
}

fn encode_branch_predictor_payload(payload: &BranchPredictorCheckpointPayload) -> Vec<u8> {
    payload.encode()
}

fn encode_gshare_branch_predictor_payload(
    payload: &GShareBranchPredictorCheckpointPayload,
) -> Vec<u8> {
    payload.encode()
}

fn encode_bimode_branch_predictor_payload(
    payload: &BiModeBranchPredictorCheckpointPayload,
) -> Vec<u8> {
    payload.encode()
}

fn encode_tournament_branch_predictor_payload(
    payload: &TournamentBranchPredictorCheckpointPayload,
) -> Vec<u8> {
    payload.encode()
}

fn encode_tage_sc_l_branch_predictor_payload(
    payload: &TageScLBranchPredictorCheckpointPayload,
) -> Vec<u8> {
    payload.encode()
}

fn encode_multiperspective_perceptron_payload(
    payload: &MultiperspectivePerceptronCheckpointPayload,
) -> Vec<u8> {
    payload.encode()
}

fn encode_o3_pending_state_payload(payload: &O3PendingStateCheckpointPayload) -> Vec<u8> {
    payload.encode()
}

fn decode_branch_predictor_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<BranchPredictorCheckpointPayload, RiscvCoreCheckpointError> {
    BranchPredictorCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidBranchPredictorSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_gshare_branch_predictor_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<GShareBranchPredictorCheckpointPayload, RiscvCoreCheckpointError> {
    GShareBranchPredictorCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidGShareBranchPredictorSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_bimode_branch_predictor_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<BiModeBranchPredictorCheckpointPayload, RiscvCoreCheckpointError> {
    BiModeBranchPredictorCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidBiModeBranchPredictorSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_tournament_branch_predictor_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<TournamentBranchPredictorCheckpointPayload, RiscvCoreCheckpointError> {
    TournamentBranchPredictorCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidTournamentBranchPredictorSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_tage_sc_l_branch_predictor_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<TageScLBranchPredictorCheckpointPayload, RiscvCoreCheckpointError> {
    TageScLBranchPredictorCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidTageScLBranchPredictorSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_multiperspective_perceptron_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<MultiperspectivePerceptronCheckpointPayload, RiscvCoreCheckpointError> {
    MultiperspectivePerceptronCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidMultiperspectivePerceptronSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_o3_pending_state_payload(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<O3PendingStateCheckpointPayload, RiscvCoreCheckpointError> {
    O3PendingStateCheckpointPayload::decode(payload).map_err(|error| {
        RiscvCoreCheckpointError::InvalidO3PendingStateSnapshot {
            component: component.clone(),
            error,
        }
    })
}

fn decode_in_order_pipeline_snapshot(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<InOrderPipelineSnapshot, RiscvCoreCheckpointError> {
    InOrderPipelineCheckpointPayload::decode(payload)
        .map(InOrderPipelineCheckpointPayload::into_snapshot)
        .map_err(
            |error| RiscvCoreCheckpointError::InvalidInOrderPipelineSnapshot {
                component: component.clone(),
                error,
            },
        )
}

fn decode_pc(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Address, RiscvCoreCheckpointError> {
    if payload.len() != U64_BYTES {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: PC_CHUNK.to_string(),
            expected: U64_BYTES,
            actual: payload.len(),
        });
    }
    Ok(Address::new(u64::from_le_bytes(
        payload.try_into().expect("pc chunk size checked"),
    )))
}

fn decode_registers(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Vec<(Register, u64)>, RiscvCoreCheckpointError> {
    if payload.len() != XREG_BYTES {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: XREGS_CHUNK.to_string(),
            expected: XREG_BYTES,
            actual: payload.len(),
        });
    }

    Ok(payload
        .chunks_exact(U64_BYTES)
        .enumerate()
        .map(|(index, bytes)| {
            let register = Register::new(index as u8).expect("register index is valid");
            let value = u64::from_le_bytes(bytes.try_into().expect("xreg chunk size checked"));
            (register, value)
        })
        .collect())
}

fn decode_float_registers(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Vec<(FloatRegister, u64)>, RiscvCoreCheckpointError> {
    if payload.len() != FREG_BYTES {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: FREGS_CHUNK.to_string(),
            expected: FREG_BYTES,
            actual: payload.len(),
        });
    }

    Ok(payload
        .chunks_exact(U64_BYTES)
        .enumerate()
        .map(|(index, bytes)| {
            let register = FloatRegister::new(index as u8).expect("register index is valid");
            let value = u64::from_le_bytes(bytes.try_into().expect("freg chunk size checked"));
            (register, value)
        })
        .collect())
}

fn decode_pmp_snapshot(
    component: &CheckpointComponentId,
    payload: &[u8],
    expected_entries: usize,
) -> Result<RiscvPmpSnapshot, RiscvCoreCheckpointError> {
    if payload.len() < PMP_HEADER_BYTES {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: PMP_CHUNK.to_string(),
            expected: PMP_HEADER_BYTES,
            actual: payload.len(),
        });
    }

    let actual_entries = usize::from(u16::from_le_bytes(
        payload[0..PMP_HEADER_BYTES]
            .try_into()
            .expect("pmp header size checked"),
    ));
    if actual_entries != expected_entries {
        return Err(RiscvCoreCheckpointError::InvalidPmpEntryCount {
            component: component.clone(),
            expected: expected_entries,
            actual: actual_entries,
        });
    }

    let expected_bytes = PMP_HEADER_BYTES + actual_entries * PMP_ENTRY_BYTES;
    if payload.len() != expected_bytes {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: PMP_CHUNK.to_string(),
            expected: expected_bytes,
            actual: payload.len(),
        });
    }

    let mut entries = Vec::with_capacity(actual_entries);
    for bytes in payload[PMP_HEADER_BYTES..].chunks_exact(PMP_ENTRY_BYTES) {
        let raw_addr = u64::from_le_bytes(
            bytes[0..U64_BYTES]
                .try_into()
                .expect("pmp raw address size checked"),
        );
        let config = RiscvPmpConfig::from_bits(bytes[U64_BYTES]).map_err(|error| {
            RiscvCoreCheckpointError::InvalidPmpSnapshot {
                component: component.clone(),
                error,
            }
        })?;
        entries.push(RiscvPmpSnapshotEntry::new(raw_addr, config));
    }

    let snapshot = RiscvPmpSnapshot::new(entries).map_err(|error| {
        RiscvCoreCheckpointError::InvalidPmpSnapshot {
            component: component.clone(),
            error,
        }
    })?;
    let mut verifier = RiscvPmpTable::new(expected_entries).map_err(|error| {
        RiscvCoreCheckpointError::InvalidPmpSnapshot {
            component: component.clone(),
            error,
        }
    })?;
    verifier
        .restore(&snapshot)
        .map_err(|error| RiscvCoreCheckpointError::InvalidPmpSnapshot {
            component: component.clone(),
            error,
        })?;
    Ok(snapshot)
}
