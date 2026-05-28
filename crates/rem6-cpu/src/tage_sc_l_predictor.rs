use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::ltage_predictor::{
    LTageBranchPredictor, LTageBranchPredictorConfig, LTageBranchPredictorError,
    LTageBranchPredictorSnapshot, LTageHistory, LTagePrediction, LTageProvider, LTageRepair,
    LTageTrainingUpdate,
};
use crate::statistical_corrector::{
    StatisticalCorrector, StatisticalCorrectorBranchKind, StatisticalCorrectorConfig,
    StatisticalCorrectorError, StatisticalCorrectorHistory, StatisticalCorrectorHistoryUpdate,
    StatisticalCorrectorInput, StatisticalCorrectorPrediction, StatisticalCorrectorSnapshot,
    StatisticalCorrectorTrainingUpdate,
};
use crate::tage_predictor::TageHistoryUpdate;
use crate::CpuId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TageScLBranchPredictorError {
    ThreadCountMismatch {
        ltage_threads: usize,
        statistical_corrector_threads: usize,
    },
    InstShiftMismatch {
        ltage_inst_shift: u8,
        statistical_corrector_inst_shift: u8,
    },
    LTage(LTageBranchPredictorError),
    StatisticalCorrector(StatisticalCorrectorError),
    SnapshotConfigMismatch {
        expected: Box<TageScLBranchPredictorConfig>,
        actual: Box<TageScLBranchPredictorConfig>,
    },
}

impl fmt::Display for TageScLBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThreadCountMismatch {
                ltage_threads,
                statistical_corrector_threads,
            } => write!(
                formatter,
                "tage-sc-l thread count mismatch ltage={ltage_threads}, statistical_corrector={statistical_corrector_threads}"
            ),
            Self::InstShiftMismatch {
                ltage_inst_shift,
                statistical_corrector_inst_shift,
            } => write!(
                formatter,
                "tage-sc-l instruction shift mismatch ltage={ltage_inst_shift}, statistical_corrector={statistical_corrector_inst_shift}"
            ),
            Self::LTage(error) => write!(formatter, "tage-sc-l ltage error: {error}"),
            Self::StatisticalCorrector(error) => {
                write!(formatter, "tage-sc-l statistical corrector error: {error}")
            }
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "tage-sc-l snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
        }
    }
}

impl Error for TageScLBranchPredictorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::LTage(error) => Some(error),
            Self::StatisticalCorrector(error) => Some(error),
            Self::ThreadCountMismatch { .. }
            | Self::InstShiftMismatch { .. }
            | Self::SnapshotConfigMismatch { .. } => None,
        }
    }
}

impl From<LTageBranchPredictorError> for TageScLBranchPredictorError {
    fn from(error: LTageBranchPredictorError) -> Self {
        Self::LTage(error)
    }
}

impl From<StatisticalCorrectorError> for TageScLBranchPredictorError {
    fn from(error: StatisticalCorrectorError) -> Self {
        Self::StatisticalCorrector(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLBranchPredictorConfig {
    ltage: LTageBranchPredictorConfig,
    statistical_corrector: StatisticalCorrectorConfig,
}

impl TageScLBranchPredictorConfig {
    pub fn new(
        ltage: LTageBranchPredictorConfig,
        statistical_corrector: StatisticalCorrectorConfig,
    ) -> Result<Self, TageScLBranchPredictorError> {
        if ltage.threads() != statistical_corrector.threads() {
            return Err(TageScLBranchPredictorError::ThreadCountMismatch {
                ltage_threads: ltage.threads(),
                statistical_corrector_threads: statistical_corrector.threads(),
            });
        }
        if ltage.inst_shift() != statistical_corrector.inst_shift() {
            return Err(TageScLBranchPredictorError::InstShiftMismatch {
                ltage_inst_shift: ltage.inst_shift(),
                statistical_corrector_inst_shift: statistical_corrector.inst_shift(),
            });
        }
        Ok(Self {
            ltage,
            statistical_corrector,
        })
    }

    pub const fn ltage(&self) -> &LTageBranchPredictorConfig {
        &self.ltage
    }

    pub const fn statistical_corrector(&self) -> &StatisticalCorrectorConfig {
        &self.statistical_corrector
    }

    pub const fn threads(&self) -> usize {
        self.ltage.threads()
    }

    pub const fn inst_shift(&self) -> u8 {
        self.ltage.inst_shift()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLBranchPredictor {
    config: TageScLBranchPredictorConfig,
    ltage: LTageBranchPredictor,
    statistical_corrector: StatisticalCorrector,
    lookup_count: u64,
    update_count: u64,
    history_update_count: u64,
    repair_count: u64,
}

impl TageScLBranchPredictor {
    pub fn new(config: TageScLBranchPredictorConfig) -> Self {
        Self {
            ltage: LTageBranchPredictor::new(config.ltage().clone()),
            statistical_corrector: StatisticalCorrector::new(
                config.statistical_corrector().clone(),
            ),
            config,
            lookup_count: 0,
            update_count: 0,
            history_update_count: 0,
            repair_count: 0,
        }
    }

    pub const fn config(&self) -> &TageScLBranchPredictorConfig {
        &self.config
    }

    pub const fn ltage(&self) -> &LTageBranchPredictor {
        &self.ltage
    }

    pub fn ltage_mut(&mut self) -> &mut LTageBranchPredictor {
        &mut self.ltage
    }

    pub const fn statistical_corrector(&self) -> &StatisticalCorrector {
        &self.statistical_corrector
    }

    pub fn statistical_corrector_mut(&mut self) -> &mut StatisticalCorrector {
        &mut self.statistical_corrector
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
    ) -> Result<TageScLPrediction, TageScLBranchPredictorError> {
        let ltage_prediction = self.ltage.predict(cpu, pc, conditional)?;
        let sc_input = self.statistical_corrector_input(&ltage_prediction);
        let statistical_corrector_prediction =
            self.statistical_corrector
                .predict(cpu, pc, conditional, sc_input)?;

        let provider = if conditional && statistical_corrector_prediction.used_sc_prediction() {
            TageScLProvider::StatisticalCorrector
        } else {
            TageScLProvider::LTage(ltage_prediction.provider())
        };
        let predicted_taken = statistical_corrector_prediction.predicted_taken();
        let history = TageScLHistory {
            cpu,
            pc,
            conditional,
            provider,
            predicted_taken,
            ltage_history: ltage_prediction.history().clone(),
            statistical_corrector_history: statistical_corrector_prediction.history().clone(),
        };

        self.lookup_count += 1;

        Ok(TageScLPrediction {
            history,
            ltage_prediction,
            statistical_corrector_prediction,
            lookup_count: self.lookup_count,
        })
    }

    pub fn train(
        &mut self,
        history: &TageScLHistory,
        actual_taken: bool,
        kind: StatisticalCorrectorBranchKind,
        target: Address,
    ) -> Result<TageScLTrainingUpdate, TageScLBranchPredictorError> {
        self.ltage.validate_train(history.ltage_history())?;

        let statistical_corrector_update = self
            .statistical_corrector
            .train(history.statistical_corrector_history(), actual_taken)?;
        let ltage_update = self
            .ltage
            .train(history.ltage_history(), actual_taken, target)?;
        let path_history = self.current_tage_path_history(history.cpu());
        let statistical_corrector_history_update = self.statistical_corrector.update_history(
            history.statistical_corrector_history(),
            kind,
            actual_taken,
            target,
            path_history,
        )?;

        self.update_count += 1;

        Ok(TageScLTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            predicted_taken: history.predicted_taken(),
            provider: history.provider(),
            statistical_corrector_update,
            ltage_update,
            statistical_corrector_history_update,
            update_count: self.update_count,
        })
    }

    pub fn update_history(
        &mut self,
        history: &TageScLHistory,
        taken: bool,
        kind: StatisticalCorrectorBranchKind,
        target: Address,
    ) -> Result<TageScLHistoryUpdate, TageScLBranchPredictorError> {
        let tage_history_update = self
            .ltage
            .tage_mut()
            .update_history(history.ltage_history().tage_history(), taken, target)
            .map_err(|error| {
                TageScLBranchPredictorError::LTage(LTageBranchPredictorError::Tage(error))
            })?;
        let path_history = self.current_tage_path_history(history.cpu());
        let statistical_corrector_history_update = self.statistical_corrector.update_history(
            history.statistical_corrector_history(),
            kind,
            taken,
            target,
            path_history,
        )?;

        self.history_update_count += 1;

        Ok(TageScLHistoryUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            tage_history_update,
            statistical_corrector_history_update,
            history_update_count: self.history_update_count,
        })
    }

    pub fn repair(
        &mut self,
        history: &TageScLHistory,
        actual_taken: bool,
        kind: StatisticalCorrectorBranchKind,
        target: Address,
    ) -> Result<TageScLRepair, TageScLBranchPredictorError> {
        let ltage_repair = self
            .ltage
            .repair(history.ltage_history(), actual_taken, target)?;
        let path_history = self.current_tage_path_history(history.cpu());
        let statistical_corrector_repair = self.statistical_corrector.repair_history(
            history.statistical_corrector_history(),
            kind,
            actual_taken,
            target,
            path_history,
        )?;

        self.repair_count += 1;

        Ok(TageScLRepair {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            ltage_repair,
            statistical_corrector_repair,
            repair_count: self.repair_count,
        })
    }

    pub fn snapshot(&self) -> TageScLBranchPredictorSnapshot {
        TageScLBranchPredictorSnapshot {
            config: self.config.clone(),
            ltage: self.ltage.snapshot(),
            statistical_corrector: self.statistical_corrector.snapshot(),
            lookup_count: self.lookup_count,
            update_count: self.update_count,
            history_update_count: self.history_update_count,
            repair_count: self.repair_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &TageScLBranchPredictorSnapshot,
    ) -> Result<(), TageScLBranchPredictorError> {
        if self.config != snapshot.config {
            return Err(TageScLBranchPredictorError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        self.ltage.restore(&snapshot.ltage)?;
        self.statistical_corrector
            .restore(&snapshot.statistical_corrector)?;
        self.lookup_count = snapshot.lookup_count;
        self.update_count = snapshot.update_count;
        self.history_update_count = snapshot.history_update_count;
        self.repair_count = snapshot.repair_count;
        Ok(())
    }

    fn statistical_corrector_input(
        &self,
        ltage_prediction: &LTagePrediction,
    ) -> StatisticalCorrectorInput {
        let tage_prediction = ltage_prediction.tage_prediction();
        StatisticalCorrectorInput::new(ltage_prediction.predicted_taken())
            .with_bias_bit(
                tage_prediction.longest_match_predicted_taken()
                    != tage_prediction.alternate_predicted_taken(),
            )
            .with_banks(
                tage_prediction.hit_bank().unwrap_or(0),
                tage_prediction.alternate_bank().unwrap_or(0),
            )
    }

    fn current_tage_path_history(&self, cpu: CpuId) -> u64 {
        self.ltage.tage().snapshot().threads()[cpu.get() as usize].path_history() as u64
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TageScLProvider {
    LTage(LTageProvider),
    StatisticalCorrector,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLPrediction {
    history: TageScLHistory,
    ltage_prediction: LTagePrediction,
    statistical_corrector_prediction: StatisticalCorrectorPrediction,
    lookup_count: u64,
}

impl TageScLPrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn provider(&self) -> TageScLProvider {
        self.history.provider()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn ltage_prediction(&self) -> &LTagePrediction {
        &self.ltage_prediction
    }

    pub const fn statistical_corrector_prediction(&self) -> &StatisticalCorrectorPrediction {
        &self.statistical_corrector_prediction
    }

    pub const fn history(&self) -> &TageScLHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLHistory {
    cpu: CpuId,
    pc: Address,
    conditional: bool,
    provider: TageScLProvider,
    predicted_taken: bool,
    ltage_history: LTageHistory,
    statistical_corrector_history: StatisticalCorrectorHistory,
}

impl TageScLHistory {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn conditional(&self) -> bool {
        self.conditional
    }

    pub const fn provider(&self) -> TageScLProvider {
        self.provider
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn ltage_history(&self) -> &LTageHistory {
        &self.ltage_history
    }

    pub const fn statistical_corrector_history(&self) -> &StatisticalCorrectorHistory {
        &self.statistical_corrector_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    predicted_taken: bool,
    provider: TageScLProvider,
    statistical_corrector_update: StatisticalCorrectorTrainingUpdate,
    ltage_update: LTageTrainingUpdate,
    statistical_corrector_history_update: StatisticalCorrectorHistoryUpdate,
    update_count: u64,
}

impl TageScLTrainingUpdate {
    pub const fn statistical_corrector_update(&self) -> &StatisticalCorrectorTrainingUpdate {
        &self.statistical_corrector_update
    }

    pub const fn ltage_update(&self) -> &LTageTrainingUpdate {
        &self.ltage_update
    }

    pub const fn statistical_corrector_history_update(&self) -> &StatisticalCorrectorHistoryUpdate {
        &self.statistical_corrector_history_update
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLHistoryUpdate {
    cpu: CpuId,
    pc: Address,
    tage_history_update: TageHistoryUpdate,
    statistical_corrector_history_update: StatisticalCorrectorHistoryUpdate,
    history_update_count: u64,
}

impl TageScLHistoryUpdate {
    pub const fn tage_history_update(&self) -> &TageHistoryUpdate {
        &self.tage_history_update
    }

    pub const fn statistical_corrector_history_update(&self) -> &StatisticalCorrectorHistoryUpdate {
        &self.statistical_corrector_history_update
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLRepair {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    ltage_repair: LTageRepair,
    statistical_corrector_repair: StatisticalCorrectorHistoryUpdate,
    repair_count: u64,
}

impl TageScLRepair {
    pub const fn ltage_repair(&self) -> &LTageRepair {
        &self.ltage_repair
    }

    pub const fn statistical_corrector_repair(&self) -> &StatisticalCorrectorHistoryUpdate {
        &self.statistical_corrector_repair
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLBranchPredictorSnapshot {
    config: TageScLBranchPredictorConfig,
    ltage: LTageBranchPredictorSnapshot,
    statistical_corrector: StatisticalCorrectorSnapshot,
    lookup_count: u64,
    update_count: u64,
    history_update_count: u64,
    repair_count: u64,
}

impl TageScLBranchPredictorSnapshot {
    pub const fn config(&self) -> &TageScLBranchPredictorConfig {
        &self.config
    }

    pub const fn ltage(&self) -> &LTageBranchPredictorSnapshot {
        &self.ltage
    }

    pub const fn statistical_corrector(&self) -> &StatisticalCorrectorSnapshot {
        &self.statistical_corrector
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn history_update_count(&self) -> u64 {
        self.history_update_count
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }
}
