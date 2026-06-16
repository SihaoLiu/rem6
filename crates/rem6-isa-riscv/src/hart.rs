use crate::{
    FloatRegister, Register, RiscvControlFlowSnapshot, RiscvControlFlowUpdate, RiscvCounterBank,
    RiscvCounterSnapshot, RiscvFloatStatus, RiscvInterruptCsr, RiscvPrivilegeMode, RiscvStatusWord,
    RiscvSv39AccessContext, RiscvVectorConfig, RiscvVectorFixedPointState,
    RiscvVectorFixedRoundingMode, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvHartState {
    pub(crate) pc: u64,
    pub(crate) hart_id: u64,
    pub(crate) counters: RiscvCounterBank,
    pub(crate) supervisor_trap_vector: u64,
    pub(crate) supervisor_scratch: u64,
    pub(crate) supervisor_exception_pc: u64,
    pub(crate) supervisor_trap_cause: u64,
    pub(crate) supervisor_trap_value: u64,
    pub(crate) machine_exception_delegation: u64,
    pub(crate) machine_interrupt_delegation: u64,
    pub(crate) machine_interrupt_enable: u64,
    pub(crate) machine_interrupt_pending: u64,
    pub(crate) machine_trap_vector: u64,
    pub(crate) machine_scratch: u64,
    pub(crate) machine_exception_pc: u64,
    pub(crate) machine_trap_cause: u64,
    pub(crate) machine_trap_value: u64,
    pub(crate) translation_satp: u64,
    pub(crate) privilege_mode: RiscvPrivilegeMode,
    pub(crate) status: RiscvStatusWord,
    pub(crate) float_status: RiscvFloatStatus,
    pub(crate) vector_config: RiscvVectorConfig,
    pub(crate) vector_fixed_point: RiscvVectorFixedPointState,
    pub(crate) registers: [u64; 32],
    pub(crate) float_registers: [u64; 32],
    pub(crate) vector_registers: [[u8; RISCV_VECTOR_REGISTER_BYTES]; 32],
}

impl RiscvHartState {
    pub const fn new(pc: u64) -> Self {
        Self::with_hart_id(pc, 0)
    }

    pub const fn with_hart_id(pc: u64, hart_id: u64) -> Self {
        Self {
            pc,
            hart_id,
            counters: RiscvCounterBank::new(),
            supervisor_trap_vector: 0,
            supervisor_scratch: 0,
            supervisor_exception_pc: 0,
            supervisor_trap_cause: 0,
            supervisor_trap_value: 0,
            machine_exception_delegation: 0,
            machine_interrupt_delegation: 0,
            machine_interrupt_enable: 0,
            machine_interrupt_pending: 0,
            machine_trap_vector: 0,
            machine_scratch: 0,
            machine_exception_pc: 0,
            machine_trap_cause: 0,
            machine_trap_value: 0,
            translation_satp: 0,
            privilege_mode: RiscvPrivilegeMode::Machine,
            status: RiscvStatusWord::new(0),
            float_status: RiscvFloatStatus::new(0),
            vector_config: RiscvVectorConfig::invalid(),
            vector_fixed_point: RiscvVectorFixedPointState::new(
                RiscvVectorFixedRoundingMode::RoundNearestUp,
            ),
            registers: [0; 32],
            float_registers: [0; 32],
            vector_registers: [[0; RISCV_VECTOR_REGISTER_BYTES]; 32],
        }
    }

    pub const fn pc(&self) -> u64 {
        self.pc
    }

    pub const fn hart_id(&self) -> u64 {
        self.hart_id
    }

    pub const fn counter_snapshot(&self) -> RiscvCounterSnapshot {
        self.counters.snapshot()
    }

    pub const fn translation_satp(&self) -> u64 {
        self.translation_satp
    }

    pub const fn translation_address_space(&self) -> u16 {
        ((self.translation_satp >> 44) & 0xffff) as u16
    }

    pub const fn privilege_mode(&self) -> RiscvPrivilegeMode {
        self.privilege_mode
    }

    pub const fn status(&self) -> RiscvStatusWord {
        self.status
    }

    pub const fn float_status(&self) -> RiscvFloatStatus {
        self.float_status
    }

    pub const fn supervisor_trap_vector(&self) -> u64 {
        self.supervisor_trap_vector
    }

    pub const fn supervisor_scratch(&self) -> u64 {
        self.supervisor_scratch
    }

    pub const fn supervisor_exception_pc(&self) -> u64 {
        self.supervisor_exception_pc
    }

    pub const fn supervisor_trap_cause(&self) -> u64 {
        self.supervisor_trap_cause
    }

    pub const fn supervisor_trap_value(&self) -> u64 {
        self.supervisor_trap_value
    }

    pub const fn machine_trap_vector(&self) -> u64 {
        self.machine_trap_vector
    }

    pub const fn machine_scratch(&self) -> u64 {
        self.machine_scratch
    }

    pub const fn machine_exception_delegation(&self) -> u64 {
        self.machine_exception_delegation
    }

    pub const fn machine_interrupt_delegation(&self) -> u64 {
        self.machine_interrupt_delegation
    }

    pub const fn machine_interrupt_enable(&self) -> u64 {
        self.machine_interrupt_enable
    }

    pub const fn machine_interrupt_pending(&self) -> u64 {
        self.machine_interrupt_pending
    }

    pub const fn machine_exception_pc(&self) -> u64 {
        self.machine_exception_pc
    }

    pub const fn machine_trap_cause(&self) -> u64 {
        self.machine_trap_cause
    }

    pub const fn machine_trap_value(&self) -> u64 {
        self.machine_trap_value
    }

    pub const fn sv39_access_context(&self) -> RiscvSv39AccessContext {
        self.sv39_access_context_for(self.privilege_mode)
    }

    pub const fn data_sv39_access_context(&self) -> RiscvSv39AccessContext {
        let privilege =
            if matches!(self.privilege_mode, RiscvPrivilegeMode::Machine) && self.status.mprv() {
                self.status.mpp()
            } else {
                self.privilege_mode
            };
        self.sv39_access_context_for(privilege)
    }

    pub fn set_privilege_mode(&mut self, privilege: RiscvPrivilegeMode) {
        self.privilege_mode = privilege;
    }

    pub fn set_status(&mut self, status: RiscvStatusWord) {
        self.status = status;
    }

    pub fn set_float_status(&mut self, status: RiscvFloatStatus) {
        self.float_status = status;
    }

    pub(crate) fn raise_float_exception_flags(&mut self, flags: u64) {
        self.float_status.raise_exception_flags(flags);
    }

    pub fn set_translation_satp(&mut self, value: u64) {
        self.translation_satp = value;
    }

    pub fn set_translation_address_space(&mut self, address_space: u16) {
        self.translation_satp =
            (self.translation_satp & !(0xffff_u64 << 44)) | (u64::from(address_space) << 44);
    }

    pub fn set_supervisor_trap_vector(&mut self, vector: u64) {
        self.supervisor_trap_vector = vector;
    }

    pub fn set_supervisor_scratch(&mut self, value: u64) {
        self.supervisor_scratch = value;
    }

    pub fn set_supervisor_exception_pc(&mut self, pc: u64) {
        self.supervisor_exception_pc = pc;
    }

    pub fn set_supervisor_trap_cause(&mut self, cause: u64) {
        self.supervisor_trap_cause = cause;
    }

    pub fn set_supervisor_trap_value(&mut self, value: u64) {
        self.supervisor_trap_value = value;
    }

    pub fn set_machine_trap_vector(&mut self, vector: u64) {
        self.machine_trap_vector = vector;
    }

    pub fn set_machine_scratch(&mut self, value: u64) {
        self.machine_scratch = value;
    }

    pub fn set_machine_exception_delegation(&mut self, delegation: u64) {
        self.machine_exception_delegation = delegation;
    }

    pub fn set_machine_interrupt_delegation(&mut self, delegation: u64) {
        self.machine_interrupt_delegation = delegation;
    }

    pub fn set_machine_interrupt_enable(&mut self, enable: u64) {
        self.machine_interrupt_enable = enable;
    }

    pub fn set_machine_interrupt_pending(&mut self, pending: u64) {
        self.machine_interrupt_pending = pending;
    }

    pub(crate) const fn read_interrupt_csr(&self, csr: RiscvInterruptCsr) -> u64 {
        match csr {
            RiscvInterruptCsr::MachineInterruptEnable => self.machine_interrupt_enable,
            RiscvInterruptCsr::MachineInterruptPending => self.machine_interrupt_pending,
            RiscvInterruptCsr::SupervisorInterruptEnable => {
                self.machine_interrupt_enable & self.machine_interrupt_delegation
            }
            RiscvInterruptCsr::SupervisorInterruptPending => {
                self.machine_interrupt_pending & self.machine_interrupt_delegation
            }
        }
    }

    pub(crate) fn write_interrupt_csr(&mut self, csr: RiscvInterruptCsr, value: u64) {
        match csr {
            RiscvInterruptCsr::MachineInterruptEnable => self.machine_interrupt_enable = value,
            RiscvInterruptCsr::MachineInterruptPending => self.machine_interrupt_pending = value,
            RiscvInterruptCsr::SupervisorInterruptEnable => {
                let mask = self.machine_interrupt_delegation;
                self.machine_interrupt_enable =
                    (self.machine_interrupt_enable & !mask) | (value & mask);
            }
            RiscvInterruptCsr::SupervisorInterruptPending => {
                let mask = self.machine_interrupt_delegation;
                self.machine_interrupt_pending =
                    (self.machine_interrupt_pending & !mask) | (value & mask);
            }
        }
    }

    pub fn set_machine_exception_pc(&mut self, pc: u64) {
        self.machine_exception_pc = pc;
    }

    pub fn set_machine_trap_cause(&mut self, cause: u64) {
        self.machine_trap_cause = cause;
    }

    pub fn set_machine_trap_value(&mut self, value: u64) {
        self.machine_trap_value = value;
    }

    pub fn set_sv39_access_context(&mut self, context: RiscvSv39AccessContext) {
        self.privilege_mode = context.privilege();
        self.status = self.status.with_mxr(context.mxr()).with_sum(context.sum());
    }

    const fn sv39_access_context_for(
        &self,
        privilege: RiscvPrivilegeMode,
    ) -> RiscvSv39AccessContext {
        RiscvSv39AccessContext::new(privilege)
            .with_mxr(self.status.mxr())
            .with_sum(self.status.sum())
    }

    pub fn set_pc(&mut self, pc: u64) {
        self.pc = pc;
    }

    pub const fn vector_config(&self) -> RiscvVectorConfig {
        self.vector_config
    }

    pub fn set_vector_config(&mut self, vector_config: RiscvVectorConfig) {
        self.vector_config = vector_config;
    }

    pub const fn vector_fixed_point(&self) -> RiscvVectorFixedPointState {
        self.vector_fixed_point
    }

    pub fn set_vector_fixed_point(&mut self, state: RiscvVectorFixedPointState) {
        self.vector_fixed_point = state;
    }

    pub const fn control_flow_snapshot(&self) -> RiscvControlFlowSnapshot {
        RiscvControlFlowSnapshot::new(self.pc, self.vector_config)
    }

    pub fn apply_control_flow_update(&mut self, update: RiscvControlFlowUpdate) {
        match update {
            RiscvControlFlowUpdate::BranchPrediction(target) => {
                self.pc = target.pc();
            }
            RiscvControlFlowUpdate::VectorConfig(update) => {
                self.pc = update.pc();
                self.vector_config = update.vector_config();
            }
        }
    }

    pub fn read(&self, register: Register) -> u64 {
        if register.is_zero() {
            0
        } else {
            self.registers[register.index() as usize]
        }
    }

    pub fn write(&mut self, register: Register, value: u64) {
        if !register.is_zero() {
            self.registers[register.index() as usize] = value;
        }
    }

    pub fn read_float(&self, register: FloatRegister) -> u64 {
        self.float_registers[register.index() as usize]
    }

    pub fn write_float(&mut self, register: FloatRegister, value: u64) {
        self.float_registers[register.index() as usize] = value;
    }

    pub const fn read_vector(&self, register: VectorRegister) -> [u8; RISCV_VECTOR_REGISTER_BYTES] {
        self.vector_registers[register.index() as usize]
    }

    pub fn write_vector(
        &mut self,
        register: VectorRegister,
        value: [u8; RISCV_VECTOR_REGISTER_BYTES],
    ) {
        self.vector_registers[register.index() as usize] = value;
    }
}
