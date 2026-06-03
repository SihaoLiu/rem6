use crate::{
    Register, RiscvControlFlowSnapshot, RiscvControlFlowUpdate, RiscvCounterSnapshot,
    RiscvHartState, RiscvPrivilegeMode, RiscvStatusWord, RiscvSv39AccessContext, RiscvVectorConfig,
};

impl RiscvHartState {
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

    pub const fn supervisor_trap_vector(&self) -> u64 {
        self.supervisor_trap_vector
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
}
