use crate::{RiscvHartState, RiscvPrivilegeMode, RiscvStatusWord, RiscvSv39AccessContext};

impl RiscvHartState {
    pub const fn privilege_mode(&self) -> RiscvPrivilegeMode {
        self.privilege_mode
    }

    pub const fn status(&self) -> RiscvStatusWord {
        self.status
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
}
