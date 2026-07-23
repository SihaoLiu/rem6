use rem6_isa_riscv::{MemoryWidth, Register, RiscvInstruction};

use crate::{
    riscv_fetch_ahead::{
        O3MemoryResultWindowAuthorization, O3MemoryResultWindowRole, O3MemoryResultWindowRoute,
    },
    riscv_live_retire_window::RiscvCompletedFetchInstruction,
    RiscvCoreState,
};

pub(in crate::riscv_fetch_ahead) struct DependentResultAddressAuthorizer {
    row_limit: usize,
    head_destination: Register,
    previous_pending_destination: Option<Register>,
    result_destinations: Vec<Register>,
    dependent_rows: usize,
}

impl DependentResultAddressAuthorizer {
    pub(in crate::riscv_fetch_ahead) fn from_head(
        state: &RiscvCoreState,
        head: &RiscvCompletedFetchInstruction,
        head_authorization: O3MemoryResultWindowAuthorization,
        row_limit: usize,
    ) -> Option<Self> {
        if !state.live_retire_gate.detailed_policy_enabled()
            || state.data_translation.is_some()
            || row_limit < 2
            || head_authorization.role() != O3MemoryResultWindowRole::Head
            || head_authorization.route() != O3MemoryResultWindowRoute::Memory
            || head_authorization.resolved_range().is_none()
        {
            return None;
        }
        let head_destination = match head.decoded().instruction() {
            RiscvInstruction::Load {
                rd,
                width: MemoryWidth::Doubleword,
                ..
            } if !rd.is_zero() => rd,
            RiscvInstruction::AtomicMemory {
                rd,
                acquire: false,
                release: false,
                ..
            } if !rd.is_zero() => rd,
            _ => return None,
        };
        if head_authorization.integer_destination() != Some(head_destination) {
            return None;
        }
        let mut result_destinations = Vec::with_capacity(row_limit.saturating_add(1));
        result_destinations.push(head_destination);
        Some(Self {
            row_limit,
            head_destination,
            previous_pending_destination: None,
            result_destinations,
            dependent_rows: 0,
        })
    }

    pub(in crate::riscv_fetch_ahead) fn try_authorize_next(
        &mut self,
        younger: &RiscvCompletedFetchInstruction,
    ) -> Option<O3MemoryResultWindowAuthorization> {
        if self.dependent_rows >= 3 || (self.dependent_rows >= 1 && self.row_limit < 4) {
            return None;
        }
        let RiscvInstruction::Load {
            rd,
            rs1,
            offset,
            width: MemoryWidth::Doubleword,
            ..
        } = younger.decoded().instruction()
        else {
            return None;
        };
        if younger.decoded().bytes() != 4
            || rd.is_zero()
            || rd == rs1
            || self.result_destinations.contains(&rd)
        {
            return None;
        }
        let allowed_source = if self.dependent_rows == 0 {
            rs1 == self.head_destination
        } else {
            rs1 == self.head_destination || Some(rs1) == self.previous_pending_destination
        };
        if !allowed_source {
            return None;
        }
        self.previous_pending_destination = Some(rd);
        self.result_destinations.push(rd);
        self.dependent_rows += 1;
        Some(O3MemoryResultWindowAuthorization::dependent(
            rd,
            rs1,
            MemoryWidth::Doubleword,
            offset,
        ))
    }

    pub(in crate::riscv_fetch_ahead) fn result_destinations(&self) -> &[Register] {
        &self.result_destinations
    }

    pub(in crate::riscv_fetch_ahead) const fn dependent_rows(&self) -> usize {
        self.dependent_rows
    }
}

pub(in crate::riscv_fetch_ahead) fn dependent_result_address_authorization(
    state: &RiscvCoreState,
    head: &RiscvCompletedFetchInstruction,
    younger: &RiscvCompletedFetchInstruction,
    head_authorization: O3MemoryResultWindowAuthorization,
    row_limit: usize,
) -> Option<O3MemoryResultWindowAuthorization> {
    let mut authorizer =
        DependentResultAddressAuthorizer::from_head(state, head, head_authorization, row_limit)?;
    let authorization = authorizer.try_authorize_next(younger)?;
    let dependent_rows = authorizer.dependent_rows();
    let result_destinations = authorizer.result_destinations();
    debug_assert_eq!(dependent_rows, 1);
    debug_assert_eq!(
        result_destinations.last().copied(),
        authorization.integer_destination()
    );
    Some(authorization)
}
