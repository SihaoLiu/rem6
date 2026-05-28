use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::loop_predictor::{
    LoopBranchPredictor, LoopBranchPredictorConfig, LoopBranchPredictorError,
    LoopBranchPredictorSnapshot, LoopHistory, LoopPrediction, LoopSquash, LoopTrainingUpdate,
};
use crate::tage_predictor::{
    TageBranchPredictor, TageBranchPredictorConfig, TageBranchPredictorError,
    TageBranchPredictorSnapshot, TageHistory, TageHistoryUpdate, TagePrediction, TageProvider,
    TageTrainingUpdate,
};
use crate::CpuId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LTageBranchPredictorError {
    ThreadCountMismatch {
        tage_threads: usize,
        loop_threads: usize,
    },
    InstShiftMismatch {
        tage_inst_shift: u8,
        loop_inst_shift: u8,
    },
    Tage(TageBranchPredictorError),
    Loop(LoopBranchPredictorError),
    SnapshotConfigMismatch {
        expected: Box<LTageBranchPredictorConfig>,
        actual: Box<LTageBranchPredictorConfig>,
    },
}

impl fmt::Display for LTageBranchPredictorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ThreadCountMismatch {
                tage_threads,
                loop_threads,
            } => write!(
                formatter,
                "ltage thread count mismatch tage={tage_threads}, loop={loop_threads}"
            ),
            Self::InstShiftMismatch {
                tage_inst_shift,
                loop_inst_shift,
            } => write!(
                formatter,
                "ltage instruction shift mismatch tage={tage_inst_shift}, loop={loop_inst_shift}"
            ),
            Self::Tage(error) => write!(formatter, "ltage tage error: {error}"),
            Self::Loop(error) => write!(formatter, "ltage loop predictor error: {error}"),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "ltage snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
        }
    }
}

impl Error for LTageBranchPredictorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Tage(error) => Some(error),
            Self::Loop(error) => Some(error),
            Self::ThreadCountMismatch { .. }
            | Self::InstShiftMismatch { .. }
            | Self::SnapshotConfigMismatch { .. } => None,
        }
    }
}

impl From<TageBranchPredictorError> for LTageBranchPredictorError {
    fn from(error: TageBranchPredictorError) -> Self {
        Self::Tage(error)
    }
}

impl From<LoopBranchPredictorError> for LTageBranchPredictorError {
    fn from(error: LoopBranchPredictorError) -> Self {
        Self::Loop(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTageBranchPredictorConfig {
    tage: TageBranchPredictorConfig,
    loop_predictor: LoopBranchPredictorConfig,
}

impl LTageBranchPredictorConfig {
    pub fn new(
        tage: TageBranchPredictorConfig,
        loop_predictor: LoopBranchPredictorConfig,
    ) -> Result<Self, LTageBranchPredictorError> {
        if tage.threads() != loop_predictor.threads() {
            return Err(LTageBranchPredictorError::ThreadCountMismatch {
                tage_threads: tage.threads(),
                loop_threads: loop_predictor.threads(),
            });
        }
        if tage.inst_shift() != loop_predictor.inst_shift() {
            return Err(LTageBranchPredictorError::InstShiftMismatch {
                tage_inst_shift: tage.inst_shift(),
                loop_inst_shift: loop_predictor.inst_shift(),
            });
        }
        Ok(Self {
            tage,
            loop_predictor,
        })
    }

    pub const fn tage(&self) -> &TageBranchPredictorConfig {
        &self.tage
    }

    pub const fn loop_predictor(&self) -> &LoopBranchPredictorConfig {
        &self.loop_predictor
    }

    pub const fn threads(&self) -> usize {
        self.tage.threads()
    }

    pub const fn inst_shift(&self) -> u8 {
        self.tage.inst_shift()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTageBranchPredictor {
    config: LTageBranchPredictorConfig,
    tage: TageBranchPredictor,
    loop_predictor: LoopBranchPredictor,
    lookup_count: u64,
    update_count: u64,
    repair_count: u64,
}

impl LTageBranchPredictor {
    pub fn new(config: LTageBranchPredictorConfig) -> Self {
        Self {
            tage: TageBranchPredictor::new(config.tage().clone()),
            loop_predictor: LoopBranchPredictor::new(config.loop_predictor().clone()),
            config,
            lookup_count: 0,
            update_count: 0,
            repair_count: 0,
        }
    }

    pub const fn config(&self) -> &LTageBranchPredictorConfig {
        &self.config
    }

    pub const fn tage(&self) -> &TageBranchPredictor {
        &self.tage
    }

    pub fn tage_mut(&mut self) -> &mut TageBranchPredictor {
        &mut self.tage
    }

    pub const fn loop_predictor(&self) -> &LoopBranchPredictor {
        &self.loop_predictor
    }

    pub fn loop_predictor_mut(&mut self) -> &mut LoopBranchPredictor {
        &mut self.loop_predictor
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
    ) -> Result<LTagePrediction, LTageBranchPredictorError> {
        let tage_prediction = self.tage.predict(cpu, pc, conditional)?;
        let loop_prediction =
            self.loop_predictor
                .predict(cpu, pc, conditional, tage_prediction.predicted_taken())?;
        let provider = if conditional && loop_prediction.loop_prediction_used() {
            LTageProvider::Loop
        } else {
            LTageProvider::Tage(tage_prediction.provider())
        };
        let predicted_taken = loop_prediction.predicted_taken();
        let history = LTageHistory {
            cpu,
            pc,
            conditional,
            provider,
            predicted_taken,
            tage_history: tage_prediction.history().clone(),
            loop_history: loop_prediction.history().clone(),
        };

        self.lookup_count += 1;

        Ok(LTagePrediction {
            history,
            tage_prediction,
            loop_prediction,
            lookup_count: self.lookup_count,
        })
    }

    pub fn train(
        &mut self,
        history: &LTageHistory,
        actual_taken: bool,
        target: Address,
    ) -> Result<LTageTrainingUpdate, LTageBranchPredictorError> {
        self.validate_train(history)?;

        let loop_update = self
            .loop_predictor
            .train(history.loop_history(), actual_taken)?;
        let tage_update = self.tage.train(history.tage_history(), actual_taken)?;
        let history_update =
            self.tage
                .update_history(history.tage_history(), actual_taken, target)?;

        self.update_count += 1;

        Ok(LTageTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            predicted_taken: history.predicted_taken(),
            provider: history.provider(),
            loop_update,
            tage_update,
            history_update,
            update_count: self.update_count,
        })
    }

    pub fn validate_train(&self, history: &LTageHistory) -> Result<(), LTageBranchPredictorError> {
        self.tage.validate_history_update(history.tage_history())?;
        Ok(())
    }

    pub fn repair(
        &mut self,
        history: &LTageHistory,
        actual_taken: bool,
        target: Address,
    ) -> Result<LTageRepair, LTageBranchPredictorError> {
        let history_update =
            self.tage
                .repair_history(history.tage_history(), actual_taken, target)?;
        let loop_squash = self.loop_predictor.squash(history.loop_history())?;

        self.repair_count += 1;

        Ok(LTageRepair {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            history_update,
            loop_squash,
            repair_count: self.repair_count,
        })
    }

    pub fn snapshot(&self) -> LTageBranchPredictorSnapshot {
        LTageBranchPredictorSnapshot {
            config: self.config.clone(),
            tage: self.tage.snapshot(),
            loop_predictor: self.loop_predictor.snapshot(),
            lookup_count: self.lookup_count,
            update_count: self.update_count,
            repair_count: self.repair_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &LTageBranchPredictorSnapshot,
    ) -> Result<(), LTageBranchPredictorError> {
        if self.config != snapshot.config {
            return Err(LTageBranchPredictorError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }

        self.tage.restore(&snapshot.tage)?;
        self.loop_predictor.restore(&snapshot.loop_predictor)?;
        self.lookup_count = snapshot.lookup_count;
        self.update_count = snapshot.update_count;
        self.repair_count = snapshot.repair_count;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LTageProvider {
    Tage(TageProvider),
    Loop,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTagePrediction {
    history: LTageHistory,
    tage_prediction: TagePrediction,
    loop_prediction: LoopPrediction,
    lookup_count: u64,
}

impl LTagePrediction {
    pub const fn cpu(&self) -> CpuId {
        self.history.cpu()
    }

    pub const fn pc(&self) -> Address {
        self.history.pc()
    }

    pub const fn conditional(&self) -> bool {
        self.history.conditional()
    }

    pub const fn provider(&self) -> LTageProvider {
        self.history.provider()
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn tage_prediction(&self) -> &TagePrediction {
        &self.tage_prediction
    }

    pub const fn loop_prediction(&self) -> &LoopPrediction {
        &self.loop_prediction
    }

    pub const fn history(&self) -> &LTageHistory {
        &self.history
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTageHistory {
    cpu: CpuId,
    pc: Address,
    conditional: bool,
    provider: LTageProvider,
    predicted_taken: bool,
    tage_history: TageHistory,
    loop_history: LoopHistory,
}

impl LTageHistory {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn conditional(&self) -> bool {
        self.conditional
    }

    pub const fn provider(&self) -> LTageProvider {
        self.provider
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn tage_history(&self) -> &TageHistory {
        &self.tage_history
    }

    pub const fn loop_history(&self) -> &LoopHistory {
        &self.loop_history
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTageTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    predicted_taken: bool,
    provider: LTageProvider,
    loop_update: LoopTrainingUpdate,
    tage_update: TageTrainingUpdate,
    history_update: TageHistoryUpdate,
    update_count: u64,
}

impl LTageTrainingUpdate {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn actual_taken(&self) -> bool {
        self.actual_taken
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn provider(&self) -> LTageProvider {
        self.provider
    }

    pub const fn loop_update(&self) -> &LoopTrainingUpdate {
        &self.loop_update
    }

    pub const fn tage_update(&self) -> &TageTrainingUpdate {
        &self.tage_update
    }

    pub const fn history_update(&self) -> &TageHistoryUpdate {
        &self.history_update
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTageRepair {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    history_update: TageHistoryUpdate,
    loop_squash: LoopSquash,
    repair_count: u64,
}

impl LTageRepair {
    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn actual_taken(&self) -> bool {
        self.actual_taken
    }

    pub const fn history_update(&self) -> &TageHistoryUpdate {
        &self.history_update
    }

    pub const fn loop_squash(&self) -> &LoopSquash {
        &self.loop_squash
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LTageBranchPredictorSnapshot {
    config: LTageBranchPredictorConfig,
    tage: TageBranchPredictorSnapshot,
    loop_predictor: LoopBranchPredictorSnapshot,
    lookup_count: u64,
    update_count: u64,
    repair_count: u64,
}

impl LTageBranchPredictorSnapshot {
    pub const fn config(&self) -> &LTageBranchPredictorConfig {
        &self.config
    }

    pub const fn tage(&self) -> &TageBranchPredictorSnapshot {
        &self.tage
    }

    pub const fn loop_predictor(&self) -> &LoopBranchPredictorSnapshot {
        &self.loop_predictor
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub const fn repair_count(&self) -> u64 {
        self.repair_count
    }
}
