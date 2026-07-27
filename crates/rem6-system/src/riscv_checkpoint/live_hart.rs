use rem6_checkpoint::CheckpointComponentId;
use rem6_isa_riscv::{
    FloatRegister, Register, RiscvCounterSnapshot, RiscvFloatStatus, RiscvGdbXlen, RiscvHartState,
    RiscvPrivilegeMode, RiscvStatusWord,
};

use super::vector_state::{decode_vector_architectural_state, encode_vector_architectural_state};
use super::RiscvCoreCheckpointError;

pub(super) const O3_LIVE_HART_STATE_CHUNK: &str = "o3-live-hart-state";
const MAGIC: &[u8; 4] = b"O3LH";
const VERSION: u8 = 1;

pub(super) fn encode(hart: &RiscvHartState) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(MAGIC);
    out.push(VERSION);
    out.push(match hart.xlen() {
        RiscvGdbXlen::Rv32 => 32,
        RiscvGdbXlen::Rv64 => 64,
    });
    out.push(match hart.privilege_mode() {
        RiscvPrivilegeMode::User => 0,
        RiscvPrivilegeMode::Supervisor => 1,
        RiscvPrivilegeMode::Machine => 3,
    });
    out.push(hart.float_status().bits() as u8);
    let counters = hart.counter_snapshot();
    for value in [
        hart.hart_id(),
        counters.cycle(),
        counters.time(),
        counters.instret(),
        hart.supervisor_trap_vector(),
        hart.supervisor_scratch(),
        hart.supervisor_exception_pc(),
        hart.supervisor_trap_cause(),
        hart.supervisor_trap_value(),
        hart.supervisor_environment_config(),
        hart.supervisor_counter_enable(),
        hart.machine_environment_config(),
        hart.machine_exception_delegation(),
        hart.machine_interrupt_delegation(),
        hart.machine_counter_enable(),
        hart.machine_counter_inhibit(),
        hart.machine_interrupt_enable(),
        hart.machine_interrupt_pending(),
        hart.machine_trap_vector(),
        hart.machine_scratch(),
        hart.machine_exception_pc(),
        hart.machine_trap_cause(),
        hart.machine_trap_value(),
        hart.translation_satp(),
        hart.status().bits(),
        hart.pc(),
    ] {
        out.extend_from_slice(&value.to_le_bytes());
    }
    for index in 0..32 {
        out.extend_from_slice(&hart.read(Register::new(index).unwrap()).to_le_bytes());
    }
    for index in 0..32 {
        out.extend_from_slice(
            &hart
                .read_float(FloatRegister::new(index).unwrap())
                .to_le_bytes(),
        );
    }
    out.extend_from_slice(&encode_vector_architectural_state(
        &hart.vector_architectural_state(),
    ));
    out
}

pub(super) fn decode(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<RiscvHartState, RiscvCoreCheckpointError> {
    let mut cursor = Cursor {
        component,
        payload,
        offset: 0,
    };
    if cursor.take(4)? != MAGIC || cursor.byte()? != VERSION {
        return Err(invalid(component, "invalid magic or version"));
    }
    let xlen = match cursor.byte()? {
        32 => RiscvGdbXlen::Rv32,
        64 => RiscvGdbXlen::Rv64,
        _ => return Err(invalid(component, "invalid XLEN")),
    };
    let privilege = match cursor.byte()? {
        0 => RiscvPrivilegeMode::User,
        1 => RiscvPrivilegeMode::Supervisor,
        3 => RiscvPrivilegeMode::Machine,
        _ => return Err(invalid(component, "invalid privilege")),
    };
    let float_status = RiscvFloatStatus::new(u64::from(cursor.byte()?));
    let mut values = [0_u64; 26];
    for value in &mut values {
        *value = cursor.u64()?;
    }
    let mut hart = RiscvHartState::with_hart_id(values[25], values[0]);
    hart.set_xlen(xlen);
    hart.set_privilege_mode(privilege);
    hart.set_float_status(float_status);
    hart.restore_counter_snapshot(&RiscvCounterSnapshot::with_time(
        values[1], values[2], values[3],
    ));
    hart.set_supervisor_trap_vector(values[4]);
    hart.set_supervisor_scratch(values[5]);
    hart.set_supervisor_exception_pc(values[6]);
    hart.set_supervisor_trap_cause(values[7]);
    hart.set_supervisor_trap_value(values[8]);
    hart.set_supervisor_environment_config(values[9]);
    hart.set_supervisor_counter_enable(values[10]);
    hart.set_machine_environment_config(values[11]);
    hart.set_machine_exception_delegation(values[12]);
    hart.set_machine_interrupt_delegation(values[13]);
    hart.set_machine_counter_enable(values[14]);
    hart.set_machine_counter_inhibit(values[15]);
    hart.set_machine_interrupt_enable(values[16]);
    hart.set_machine_interrupt_pending(values[17]);
    hart.set_machine_trap_vector(values[18]);
    hart.set_machine_scratch(values[19]);
    hart.set_machine_exception_pc(values[20]);
    hart.set_machine_trap_cause(values[21]);
    hart.set_machine_trap_value(values[22]);
    hart.set_translation_satp(values[23]);
    hart.set_status(RiscvStatusWord::new(values[24]));
    for index in 0..32 {
        hart.write(Register::new(index).unwrap(), cursor.u64()?);
    }
    for index in 0..32 {
        hart.write_float(FloatRegister::new(index).unwrap(), cursor.u64()?);
    }
    let vector = decode_vector_architectural_state(component, Some(&[1]), Some(cursor.rest()))?;
    hart.restore_vector_architectural_state(&vector);
    if encode(&hart) != payload {
        return Err(invalid(component, "noncanonical payload"));
    }
    Ok(hart)
}

fn invalid(component: &CheckpointComponentId, reason: &'static str) -> RiscvCoreCheckpointError {
    RiscvCoreCheckpointError::InvalidO3LiveHartState {
        component: component.clone(),
        reason,
    }
}

struct Cursor<'a> {
    component: &'a CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, bytes: usize) -> Result<&'a [u8], RiscvCoreCheckpointError> {
        let end = self
            .offset
            .checked_add(bytes)
            .filter(|end| *end <= self.payload.len())
            .ok_or_else(|| invalid(self.component, "truncated payload"))?;
        let value = &self.payload[self.offset..end];
        self.offset = end;
        Ok(value)
    }

    fn byte(&mut self) -> Result<u8, RiscvCoreCheckpointError> {
        Ok(self.take(1)?[0])
    }

    fn u64(&mut self) -> Result<u64, RiscvCoreCheckpointError> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn rest(&mut self) -> &'a [u8] {
        let rest = &self.payload[self.offset..];
        self.offset = self.payload.len();
        rest
    }
}
