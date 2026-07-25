use crate::{
    RiscvVectorConfig, RiscvVectorFixedPointState, RiscvVectorFixedRoundingMode, VectorRegister,
    RISCV_VECTOR_REGISTER_BYTES,
};

pub const RISCV_VECTOR_REGISTER_COUNT: usize = 32;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvVectorArchitecturalState {
    config: RiscvVectorConfig,
    fixed_point: RiscvVectorFixedPointState,
    registers: [[u8; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT],
}

impl RiscvVectorArchitecturalState {
    pub const fn new(
        config: RiscvVectorConfig,
        fixed_point: RiscvVectorFixedPointState,
        registers: [[u8; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT],
    ) -> Self {
        Self {
            config,
            fixed_point,
            registers,
        }
    }

    pub const fn config(&self) -> RiscvVectorConfig {
        self.config
    }

    pub const fn fixed_point(&self) -> RiscvVectorFixedPointState {
        self.fixed_point
    }

    pub const fn registers(
        &self,
    ) -> &[[u8; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT] {
        &self.registers
    }

    pub const fn register(&self, register: VectorRegister) -> [u8; RISCV_VECTOR_REGISTER_BYTES] {
        self.registers[register.index() as usize]
    }
}

impl Default for RiscvVectorArchitecturalState {
    fn default() -> Self {
        Self::new(
            RiscvVectorConfig::invalid(),
            RiscvVectorFixedPointState::new(RiscvVectorFixedRoundingMode::RoundNearestUp),
            [[0; RISCV_VECTOR_REGISTER_BYTES]; RISCV_VECTOR_REGISTER_COUNT],
        )
    }
}
