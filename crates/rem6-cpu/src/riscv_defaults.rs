use crate::{
    CpuId, LTageBranchPredictorConfig, LoopBranchPredictorConfig, MultiperspectivePerceptron,
    MultiperspectivePerceptronConfig, StatisticalCorrectorConfig, TageBranchPredictorConfig,
    TageScLBranchPredictor, TageScLBranchPredictorConfig,
};

pub const DEFAULT_RISCV_PMP_ENTRIES: usize = 16;
pub const MIN_RISCV_BRANCH_LOOKAHEAD: usize = 1;
pub const DEFAULT_RISCV_BRANCH_LOOKAHEAD: usize = MIN_RISCV_BRANCH_LOOKAHEAD;
pub const MAX_RISCV_BRANCH_LOOKAHEAD: usize = 3;
pub const MIN_RISCV_O3_ISSUE_WIDTH: usize = 1;
pub const DEFAULT_RISCV_O3_ISSUE_WIDTH: usize = 4;
pub const MAX_RISCV_O3_ISSUE_WIDTH: usize = 4;
pub const MIN_RISCV_O3_WRITEBACK_WIDTH: usize = 1;
pub const DEFAULT_RISCV_O3_WRITEBACK_WIDTH: usize = 1;
pub const MAX_RISCV_O3_WRITEBACK_WIDTH: usize = 4;
pub const DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES: usize = 128;
pub const DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY: usize = 4;
pub const DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES: usize = 16;
pub const DEFAULT_RISCV_GSHARE_BRANCH_PREDICTOR_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_BIMODE_CHOICE_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_BIMODE_GLOBAL_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_LOCAL_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_LOCAL_HISTORY_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_GLOBAL_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_CHOICE_ENTRIES: usize = 1024;
pub const RISCV_LOCAL_GSHARE_THREAD: CpuId = CpuId::new(0);
pub const RISCV_LOCAL_BIMODE_THREAD: CpuId = CpuId::new(0);
pub const RISCV_LOCAL_TOURNAMENT_THREAD: CpuId = CpuId::new(0);
pub const RISCV_LOCAL_TAGE_SC_L_THREAD: CpuId = CpuId::new(0);
pub const RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD: CpuId = CpuId::new(0);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RiscvBranchPredictorKind {
    #[default]
    Basic,
    GShare,
    BiMode,
    Tournament,
    TageScL,
    MultiperspectivePerceptron,
}

pub(crate) fn default_riscv_multiperspective_perceptron() -> MultiperspectivePerceptron {
    MultiperspectivePerceptron::new(
        MultiperspectivePerceptronConfig::eight_kb(1)
            .expect("default RISC-V multiperspective perceptron config is valid"),
    )
    .expect("default RISC-V multiperspective perceptron is valid")
}

pub(crate) fn default_riscv_tage_sc_l_branch_predictor() -> TageScLBranchPredictor {
    TageScLBranchPredictor::new(
        TageScLBranchPredictorConfig::new(
            LTageBranchPredictorConfig::new(
                default_riscv_tage_branch_predictor_config(),
                default_riscv_loop_branch_predictor_config(),
            )
            .expect("default RISC-V LTage branch predictor config is valid"),
            StatisticalCorrectorConfig::tage_sc_l_8kb(1, 2, false)
                .expect("default RISC-V TAGE-SC-L statistical corrector config is valid"),
        )
        .expect("default RISC-V TAGE-SC-L branch predictor config is valid"),
    )
}

fn default_riscv_tage_branch_predictor_config() -> TageBranchPredictorConfig {
    TageBranchPredictorConfig::with_options(
        1,
        2,
        2,
        6,
        vec![0, 4, 5],
        vec![4, 3, 3],
        1,
        3,
        2,
        8,
        4,
        1,
        4,
        1,
        2,
        false,
        false,
    )
    .expect("default RISC-V TAGE branch predictor config is valid")
}

fn default_riscv_loop_branch_predictor_config() -> LoopBranchPredictorConfig {
    LoopBranchPredictorConfig::with_options(
        1, 3, 1, 3, 2, 4, 4, 3, 2, false, false, false, false, 1, 3, true,
    )
    .expect("default RISC-V loop branch predictor config is valid")
}
