use std::collections::BTreeMap;

use rem6_memory::{AccessSize, Address, CacheLineLayout};

use crate::memory_access::{GpuCoalescedMemoryAccessDelta, GpuMemoryAccessKind};
use crate::GpuWorkgroupId;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuScalarRegister(u8);

impl GpuScalarRegister {
    pub const fn new(index: u8) -> Self {
        Self(index)
    }

    pub const fn index(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuIsaProgram {
    instructions: Vec<GpuIsaInstruction>,
}

impl GpuIsaProgram {
    pub const fn empty() -> Self {
        Self {
            instructions: Vec::new(),
        }
    }

    pub fn new(instructions: Vec<GpuIsaInstruction>) -> Self {
        Self { instructions }
    }

    pub fn instructions(&self) -> &[GpuIsaInstruction] {
        &self.instructions
    }

    pub(crate) fn execute(&self, workgroup: GpuWorkgroupId) -> GpuWorkgroupExecution {
        let mut state = GpuWorkgroupIsaState::empty();
        let mut coalesced_memory_accesses = Vec::new();

        for (instruction_index, instruction) in self.instructions.iter().enumerate() {
            let effect = instruction.execute(workgroup, &mut state, instruction_index);
            coalesced_memory_accesses.extend(effect.coalesced_memory_accesses);
            state.advance_pc();
        }
        GpuWorkgroupExecution::new(state, coalesced_memory_accesses)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GpuIsaInstruction {
    LoadWorkgroupId {
        dst: GpuScalarRegister,
    },
    MoveImmediate {
        dst: GpuScalarRegister,
        value: i64,
    },
    AddImmediate {
        dst: GpuScalarRegister,
        src: GpuScalarRegister,
        immediate: i64,
    },
    GlobalMemoryAccess {
        kind: GpuMemoryAccessKind,
        base: Address,
        lane_count: u32,
        lane_stride: u64,
        access_size: AccessSize,
        line_layout: CacheLineLayout,
    },
}

impl GpuIsaInstruction {
    pub const fn load_workgroup_id(dst: GpuScalarRegister) -> Self {
        Self::LoadWorkgroupId { dst }
    }

    pub const fn move_immediate(dst: GpuScalarRegister, value: i64) -> Self {
        Self::MoveImmediate { dst, value }
    }

    pub const fn add_immediate(
        dst: GpuScalarRegister,
        src: GpuScalarRegister,
        immediate: i64,
    ) -> Self {
        Self::AddImmediate {
            dst,
            src,
            immediate,
        }
    }

    pub const fn global_load(
        base: Address,
        lane_count: u32,
        lane_stride: u64,
        access_size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Self {
        Self::GlobalMemoryAccess {
            kind: GpuMemoryAccessKind::Read,
            base,
            lane_count,
            lane_stride,
            access_size,
            line_layout,
        }
    }

    pub const fn global_store(
        base: Address,
        lane_count: u32,
        lane_stride: u64,
        access_size: AccessSize,
        line_layout: CacheLineLayout,
    ) -> Self {
        Self::GlobalMemoryAccess {
            kind: GpuMemoryAccessKind::Write,
            base,
            lane_count,
            lane_stride,
            access_size,
            line_layout,
        }
    }

    fn execute(
        self,
        workgroup: GpuWorkgroupId,
        state: &mut GpuWorkgroupIsaState,
        instruction_index: usize,
    ) -> GpuInstructionEffect {
        match self {
            Self::LoadWorkgroupId { dst } => {
                state.write_scalar(dst, i64::from(workgroup.get()));
                GpuInstructionEffect::empty()
            }
            Self::MoveImmediate { dst, value } => {
                state.write_scalar(dst, value);
                GpuInstructionEffect::empty()
            }
            Self::AddImmediate {
                dst,
                src,
                immediate,
            } => {
                let value = state
                    .scalar_register(src)
                    .unwrap_or_default()
                    .wrapping_add(immediate);
                state.write_scalar(dst, value);
                GpuInstructionEffect::empty()
            }
            Self::GlobalMemoryAccess {
                kind,
                base,
                lane_count,
                lane_stride,
                access_size,
                line_layout,
            } => coalesce_memory_accesses(
                instruction_index,
                kind,
                base,
                lane_count,
                lane_stride,
                access_size,
                line_layout,
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GpuWorkgroupExecution {
    isa_state: GpuWorkgroupIsaState,
    coalesced_memory_accesses: Vec<GpuCoalescedMemoryAccessDelta>,
}

impl GpuWorkgroupExecution {
    fn new(
        isa_state: GpuWorkgroupIsaState,
        coalesced_memory_accesses: Vec<GpuCoalescedMemoryAccessDelta>,
    ) -> Self {
        Self {
            isa_state,
            coalesced_memory_accesses,
        }
    }

    pub(crate) const fn isa_state(&self) -> &GpuWorkgroupIsaState {
        &self.isa_state
    }

    pub(crate) fn coalesced_memory_accesses(&self) -> &[GpuCoalescedMemoryAccessDelta] {
        &self.coalesced_memory_accesses
    }
}

struct GpuInstructionEffect {
    coalesced_memory_accesses: Vec<GpuCoalescedMemoryAccessDelta>,
}

impl GpuInstructionEffect {
    fn empty() -> Self {
        Self {
            coalesced_memory_accesses: Vec::new(),
        }
    }
}

fn coalesce_memory_accesses(
    instruction_index: usize,
    kind: GpuMemoryAccessKind,
    base: Address,
    lane_count: u32,
    lane_stride: u64,
    access_size: AccessSize,
    line_layout: CacheLineLayout,
) -> GpuInstructionEffect {
    let mut groups = BTreeMap::new();
    for lane in 0..lane_count {
        let offset = lane_stride
            .checked_mul(u64::from(lane))
            .expect("GPU memory lane offset fits u64");
        let address = Address::new(
            base.get()
                .checked_add(offset)
                .expect("GPU memory lane address fits u64"),
        );
        record_lane_access(&mut groups, address, access_size.bytes(), line_layout);
    }

    GpuInstructionEffect {
        coalesced_memory_accesses: groups
            .into_iter()
            .map(|(line, (access_count, byte_count))| {
                GpuCoalescedMemoryAccessDelta::new(
                    instruction_index,
                    kind,
                    line,
                    access_count,
                    byte_count,
                )
            })
            .collect(),
    }
}

fn record_lane_access(
    groups: &mut BTreeMap<Address, (u32, u64)>,
    mut address: Address,
    mut remaining_bytes: u64,
    line_layout: CacheLineLayout,
) {
    while remaining_bytes != 0 {
        let line = line_layout.line_address(address);
        let line_end = line
            .get()
            .checked_add(line_layout.bytes())
            .expect("GPU memory line end fits u64");
        let bytes_in_line = remaining_bytes.min(line_end - address.get());
        let entry = groups.entry(line).or_insert((0_u32, 0_u64));
        entry.0 += 1;
        entry.1 += bytes_in_line;
        remaining_bytes -= bytes_in_line;
        if remaining_bytes != 0 {
            address = Address::new(
                address
                    .get()
                    .checked_add(bytes_in_line)
                    .expect("GPU memory lane split address fits u64"),
            );
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuWorkgroupIsaState {
    pc: usize,
    scalar_registers: Vec<(GpuScalarRegister, i64)>,
}

impl GpuWorkgroupIsaState {
    pub const fn empty() -> Self {
        Self {
            pc: 0,
            scalar_registers: Vec::new(),
        }
    }

    pub fn from_scalar_registers<const N: usize>(
        pc: usize,
        scalar_registers: [(GpuScalarRegister, i64); N],
    ) -> Self {
        let mut scalar_registers = scalar_registers.to_vec();
        scalar_registers.sort_by_key(|(register, _value)| *register);
        scalar_registers.dedup_by_key(|(register, _value)| *register);
        Self {
            pc,
            scalar_registers,
        }
    }

    pub const fn pc(&self) -> usize {
        self.pc
    }

    pub fn scalar_register(&self, register: GpuScalarRegister) -> Option<i64> {
        self.scalar_registers
            .iter()
            .find_map(|(candidate, value)| (*candidate == register).then_some(*value))
    }

    pub fn scalar_registers(&self) -> &[(GpuScalarRegister, i64)] {
        &self.scalar_registers
    }

    fn advance_pc(&mut self) {
        self.pc += 1;
    }

    fn write_scalar(&mut self, register: GpuScalarRegister, value: i64) {
        match self
            .scalar_registers
            .binary_search_by_key(&register, |(candidate, _value)| *candidate)
        {
            Ok(index) => self.scalar_registers[index].1 = value,
            Err(index) => self.scalar_registers.insert(index, (register, value)),
        }
    }
}
