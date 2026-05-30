#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorConfig {
    vl: u32,
    vtype: u64,
}

impl RiscvVectorConfig {
    pub const VILL_BIT: u64 = 1_u64 << 63;

    pub const fn new(vl: u32, vtype: u64) -> Self {
        Self { vl, vtype }
    }

    pub const fn invalid() -> Self {
        Self {
            vl: 0,
            vtype: Self::VILL_BIT,
        }
    }

    pub const fn vl(self) -> u32 {
        self.vl
    }

    pub const fn vtype(self) -> u64 {
        self.vtype
    }

    pub const fn vill(self) -> bool {
        (self.vtype & Self::VILL_BIT) != 0
    }
}

impl Default for RiscvVectorConfig {
    fn default() -> Self {
        Self::invalid()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvControlFlowSnapshot {
    pc: u64,
    vector_config: RiscvVectorConfig,
}

impl RiscvControlFlowSnapshot {
    pub const fn new(pc: u64, vector_config: RiscvVectorConfig) -> Self {
        Self { pc, vector_config }
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }

    pub const fn vector_config(self) -> RiscvVectorConfig {
        self.vector_config
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvBranchPredictionTarget {
    pc: u64,
}

impl RiscvBranchPredictionTarget {
    pub const fn new(pc: u64) -> Self {
        Self { pc }
    }

    pub const fn from_copied_dynamic_state(snapshot: RiscvControlFlowSnapshot) -> Self {
        Self { pc: snapshot.pc() }
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorConfigUpdate {
    pc: u64,
    vector_config: RiscvVectorConfig,
}

impl RiscvVectorConfigUpdate {
    pub const fn new(pc: u64, vector_config: RiscvVectorConfig) -> Self {
        Self { pc, vector_config }
    }

    pub const fn pc(self) -> u64 {
        self.pc
    }

    pub const fn vector_config(self) -> RiscvVectorConfig {
        self.vector_config
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvControlFlowUpdate {
    BranchPrediction(RiscvBranchPredictionTarget),
    VectorConfig(RiscvVectorConfigUpdate),
}

impl RiscvControlFlowUpdate {
    pub const fn branch_prediction(target: RiscvBranchPredictionTarget) -> Self {
        Self::BranchPrediction(target)
    }

    pub const fn vector_config(update: RiscvVectorConfigUpdate) -> Self {
        Self::VectorConfig(update)
    }
}
