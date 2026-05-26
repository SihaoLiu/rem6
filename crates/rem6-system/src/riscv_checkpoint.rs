use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_cpu::RiscvCore;
use rem6_isa_riscv::Register;
use rem6_memory::Address;

const PC_CHUNK: &str = "pc";
const XREGS_CHUNK: &str = "xregs";
const U64_BYTES: usize = 8;
const XREG_COUNT: usize = 32;
const XREG_BYTES: usize = XREG_COUNT * U64_BYTES;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCoreCheckpointRecord {
    component: CheckpointComponentId,
    pc: Address,
    registers: Vec<(Register, u64)>,
}

impl RiscvCoreCheckpointRecord {
    pub fn new(
        component: CheckpointComponentId,
        pc: Address,
        registers: Vec<(Register, u64)>,
    ) -> Self {
        Self {
            component,
            pc,
            registers,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub fn registers(&self) -> &[(Register, u64)] {
        &self.registers
    }

    pub fn register(&self, register: Register) -> Option<u64> {
        self.registers
            .iter()
            .find_map(|(current, value)| (*current == register).then_some(*value))
    }
}

#[derive(Clone, Debug)]
pub struct RiscvCoreCheckpointPort {
    component: CheckpointComponentId,
    core: RiscvCore,
}

impl RiscvCoreCheckpointPort {
    pub const fn new(component: CheckpointComponentId, core: RiscvCore) -> Self {
        Self { component, core }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn core(&self) -> RiscvCore {
        self.core.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<RiscvCoreCheckpointRecord, CheckpointError> {
        let record = self.capture_record();
        registry.write_chunk(
            &self.component,
            PC_CHUNK,
            record.pc().get().to_le_bytes().to_vec(),
        )?;
        registry.write_chunk(
            &self.component,
            XREGS_CHUNK,
            encode_registers(record.registers()),
        )?;
        Ok(record)
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<RiscvCoreCheckpointRecord, RiscvCoreCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_record(&record);
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<RiscvCoreCheckpointRecord, RiscvCoreCheckpointError> {
        let pc = decode_pc(
            &self.component,
            registry.chunk(&self.component, PC_CHUNK).ok_or_else(|| {
                RiscvCoreCheckpointError::MissingChunk {
                    component: self.component.clone(),
                    name: PC_CHUNK.to_string(),
                }
            })?,
        )?;
        let registers = decode_registers(
            &self.component,
            registry
                .chunk(&self.component, XREGS_CHUNK)
                .ok_or_else(|| RiscvCoreCheckpointError::MissingChunk {
                    component: self.component.clone(),
                    name: XREGS_CHUNK.to_string(),
                })?,
        )?;

        Ok(RiscvCoreCheckpointRecord::new(
            self.component.clone(),
            pc,
            registers,
        ))
    }

    fn restore_record(&self, record: &RiscvCoreCheckpointRecord) {
        self.core.redirect_pc(record.pc());
        for (register, value) in record.registers() {
            self.core.write_register(*register, *value);
        }
    }

    fn capture_record(&self) -> RiscvCoreCheckpointRecord {
        RiscvCoreCheckpointRecord::new(
            self.component.clone(),
            self.core.pc(),
            all_register_values(&self.core),
        )
    }
}

#[derive(Clone, Debug, Default)]
pub struct RiscvCoreCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, RiscvCoreCheckpointPort>,
}

impl RiscvCoreCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = RiscvCoreCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }

        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<RiscvCoreCheckpointRecord>, RiscvCoreCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            decoded.push((port, port.decode_from(registry)?));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_record(&record);
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), RiscvCoreCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunkSize {
        component: CheckpointComponentId,
        name: String,
        expected: usize,
        actual: usize,
    },
}

impl fmt::Display for RiscvCoreCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "RISC-V core checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunkSize {
                component,
                name,
                expected,
                actual,
            } => write!(
                formatter,
                "RISC-V core checkpoint component {} chunk {name} has {actual} bytes; \
                 expected {expected}",
                component.as_str()
            ),
        }
    }
}

impl Error for RiscvCoreCheckpointError {}

fn all_register_values(core: &RiscvCore) -> Vec<(Register, u64)> {
    (0..XREG_COUNT)
        .map(|index| {
            let register = Register::new(index as u8).expect("register index is valid");
            (register, core.read_register(register))
        })
        .collect()
}

fn encode_registers(registers: &[(Register, u64)]) -> Vec<u8> {
    let mut payload = vec![0; XREG_BYTES];
    for (register, value) in registers {
        let offset = usize::from(register.index()) * U64_BYTES;
        payload[offset..offset + U64_BYTES].copy_from_slice(&value.to_le_bytes());
    }
    payload
}

fn decode_pc(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Address, RiscvCoreCheckpointError> {
    if payload.len() != U64_BYTES {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: PC_CHUNK.to_string(),
            expected: U64_BYTES,
            actual: payload.len(),
        });
    }
    Ok(Address::new(u64::from_le_bytes(
        payload.try_into().expect("pc chunk size checked"),
    )))
}

fn decode_registers(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Vec<(Register, u64)>, RiscvCoreCheckpointError> {
    if payload.len() != XREG_BYTES {
        return Err(RiscvCoreCheckpointError::InvalidChunkSize {
            component: component.clone(),
            name: XREGS_CHUNK.to_string(),
            expected: XREG_BYTES,
            actual: payload.len(),
        });
    }

    Ok(payload
        .chunks_exact(U64_BYTES)
        .enumerate()
        .map(|(index, bytes)| {
            let register = Register::new(index as u8).expect("register index is valid");
            let value = u64::from_le_bytes(bytes.try_into().expect("xreg chunk size checked"));
            (register, value)
        })
        .collect())
}
