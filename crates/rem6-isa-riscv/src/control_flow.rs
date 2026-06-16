#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvVectorConfig {
    vl: u32,
    vtype: u64,
}

impl RiscvVectorConfig {
    pub const VILL_BIT: u64 = 1_u64 << 63;
    pub const DEFAULT_VLEN_BITS: u32 = 128;

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

    pub fn from_avl(vtype: u64, avl: u64) -> Self {
        let Some(vlmax) = Self::vlmax(vtype) else {
            return Self::invalid();
        };
        Self::new(avl.min(u64::from(vlmax)) as u32, vtype)
    }

    pub fn vlmax(vtype: u64) -> Option<u32> {
        if vtype & !0xff != 0 {
            return None;
        }

        let vlmul = vtype & 0x7;
        let vsew = ((vtype >> 3) & 0x7) as u32;
        if vsew > 3 {
            return None;
        }

        let base = Self::DEFAULT_VLEN_BITS / (8_u32 << vsew);
        match vlmul {
            0 => Some(base),
            1 => Some(base * 2),
            2 => Some(base * 4),
            3 => Some(base * 8),
            4 => None,
            5 => nonzero_fractional_lmul(base, 8),
            6 => nonzero_fractional_lmul(base, 4),
            7 => nonzero_fractional_lmul(base, 2),
            _ => unreachable!(),
        }
    }
}

fn nonzero_fractional_lmul(base: u32, denominator: u32) -> Option<u32> {
    let vlmax = base / denominator;
    (vlmax != 0).then_some(vlmax)
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
