use std::error::Error;
use std::fmt;

use rem6_boot::BootElfMetadata;
use rem6_memory::{AccessSize, Address, AddressRange, MemoryError};

use super::RISCV_PAGE_BYTES;

const RISCV_SE_WORD_BYTES: u64 = 8;
const RISCV_SE_STACK_ALIGN: u64 = 16;
const RISCV_SE_RANDOM_BYTES: usize = 16;

pub const RISCV_LINUX_AT_NULL: u64 = 0;
pub const RISCV_LINUX_AT_ENTRY: u64 = 9;
pub const RISCV_LINUX_AT_PHDR: u64 = 3;
pub const RISCV_LINUX_AT_PHENT: u64 = 4;
pub const RISCV_LINUX_AT_PHNUM: u64 = 5;
pub const RISCV_LINUX_AT_PAGESZ: u64 = 6;
pub const RISCV_LINUX_AT_SECURE: u64 = 23;
pub const RISCV_LINUX_AT_RANDOM: u64 = 25;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvSeAuxvEntry {
    key: u64,
    value: u64,
}

impl RiscvSeAuxvEntry {
    pub const fn new(key: u64, value: u64) -> Self {
        Self { key, value }
    }

    pub const fn key(self) -> u64 {
        self.key
    }

    pub const fn value(self) -> u64 {
        self.value
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSeStartupConfig {
    stack_top: Address,
    argv: Vec<Vec<u8>>,
    envp: Vec<Vec<u8>>,
    auxv: Vec<RiscvSeAuxvEntry>,
    random_bytes: [u8; RISCV_SE_RANDOM_BYTES],
}

impl RiscvSeStartupConfig {
    pub const fn new(stack_top: Address) -> Self {
        Self {
            stack_top,
            argv: Vec::new(),
            envp: Vec::new(),
            auxv: Vec::new(),
            random_bytes: [0; RISCV_SE_RANDOM_BYTES],
        }
    }

    pub fn with_arg(mut self, arg: impl AsRef<[u8]>) -> Self {
        self.argv.push(arg.as_ref().to_vec());
        self
    }

    pub fn with_env(mut self, env: impl AsRef<[u8]>) -> Self {
        self.envp.push(env.as_ref().to_vec());
        self
    }

    pub fn with_auxv_entry(mut self, entry: RiscvSeAuxvEntry) -> Self {
        self.auxv.push(entry);
        self
    }

    pub fn with_elf_auxv(mut self, metadata: BootElfMetadata) -> Self {
        let table = metadata.program_header_table();
        if let Some(address) = table.memory_address() {
            self.auxv
                .push(RiscvSeAuxvEntry::new(RISCV_LINUX_AT_PHDR, address.get()));
            self.auxv.push(RiscvSeAuxvEntry::new(
                RISCV_LINUX_AT_PHENT,
                u64::from(table.entry_size()),
            ));
            self.auxv.push(RiscvSeAuxvEntry::new(
                RISCV_LINUX_AT_PHNUM,
                table.entry_count(),
            ));
        }
        self
    }

    pub const fn with_random_bytes(mut self, bytes: [u8; RISCV_SE_RANDOM_BYTES]) -> Self {
        self.random_bytes = bytes;
        self
    }

    pub fn build(&self) -> Result<RiscvSeStartupImage, RiscvSeStartupError> {
        validate_strings(RiscvSeStartupStringField::Argument, &self.argv)?;
        validate_strings(RiscvSeStartupStringField::Environment, &self.envp)?;
        validate_auxv_defaults(&self.auxv)?;

        let stack_top = self.stack_top.get();
        let mut stack_min = stack_top;
        stack_min = subtract_stack_bytes(stack_min, RISCV_SE_RANDOM_BYTES as u64)?;
        let random_address = Address::new(stack_min);

        let mut argv_pointers = Vec::with_capacity(self.argv.len() + 1);
        for arg in &self.argv {
            stack_min = subtract_stack_bytes(stack_min, nul_terminated_len(arg)?)?;
            argv_pointers.push(Address::new(stack_min));
        }
        argv_pointers.push(Address::new(0));

        let mut envp_pointers = Vec::with_capacity(self.envp.len() + 1);
        for env in &self.envp {
            stack_min = subtract_stack_bytes(stack_min, nul_terminated_len(env)?)?;
            envp_pointers.push(Address::new(stack_min));
        }
        envp_pointers.push(Address::new(0));

        stack_min = align_down(stack_min, RISCV_SE_WORD_BYTES);
        let auxv = self.auxv_with_defaults(random_address);
        stack_min = subtract_stack_bytes(
            stack_min,
            frame_bytes(&argv_pointers, &envp_pointers, &auxv)?,
        )?;
        stack_min = align_down(stack_min, RISCV_SE_STACK_ALIGN);

        let stack_range = stack_range(stack_min, stack_top)?;
        let mut data = vec![
            0;
            usize::try_from(stack_range.size().bytes()).map_err(|_| {
                RiscvSeStartupError::StackImageTooLarge {
                    bytes: stack_range.size().bytes(),
                }
            })?
        ];

        write_stack_bytes(
            stack_range.start(),
            &mut data,
            random_address,
            &self.random_bytes,
        );
        for (arg, pointer) in self.argv.iter().zip(argv_pointers.iter().copied()) {
            write_stack_c_string(stack_range.start(), &mut data, pointer, arg);
        }
        for (env, pointer) in self.envp.iter().zip(envp_pointers.iter().copied()) {
            write_stack_c_string(stack_range.start(), &mut data, pointer, env);
        }

        let initial_stack_pointer = stack_range.start();
        let mut cursor = initial_stack_pointer.get();
        write_stack_u64(
            stack_range.start(),
            &mut data,
            &mut cursor,
            self.argv.len() as u64,
        );
        for pointer in argv_pointers {
            write_stack_u64(stack_range.start(), &mut data, &mut cursor, pointer.get());
        }
        for pointer in envp_pointers {
            write_stack_u64(stack_range.start(), &mut data, &mut cursor, pointer.get());
        }
        for entry in &auxv {
            write_stack_u64(stack_range.start(), &mut data, &mut cursor, entry.key());
            write_stack_u64(stack_range.start(), &mut data, &mut cursor, entry.value());
        }

        Ok(RiscvSeStartupImage {
            initial_stack_pointer,
            stack_range,
            random_address,
            stack_data: data,
        })
    }

    fn auxv_with_defaults(&self, random_address: Address) -> Vec<RiscvSeAuxvEntry> {
        let mut auxv = self.auxv.clone();
        auxv.push(RiscvSeAuxvEntry::new(
            RISCV_LINUX_AT_PAGESZ,
            RISCV_PAGE_BYTES,
        ));
        auxv.push(RiscvSeAuxvEntry::new(RISCV_LINUX_AT_SECURE, 0));
        auxv.push(RiscvSeAuxvEntry::new(
            RISCV_LINUX_AT_RANDOM,
            random_address.get(),
        ));
        auxv.push(RiscvSeAuxvEntry::new(RISCV_LINUX_AT_NULL, 0));
        auxv
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSeStartupImage {
    initial_stack_pointer: Address,
    stack_range: AddressRange,
    random_address: Address,
    stack_data: Vec<u8>,
}

impl RiscvSeStartupImage {
    pub const fn initial_stack_pointer(&self) -> Address {
        self.initial_stack_pointer
    }

    pub const fn stack_range(&self) -> AddressRange {
        self.stack_range
    }

    pub const fn random_address(&self) -> Address {
        self.random_address
    }

    pub fn stack_data(&self) -> &[u8] {
        &self.stack_data
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvSeStartupStringField {
    Argument,
    Environment,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSeStartupError {
    InteriorNul {
        field: RiscvSeStartupStringField,
        index: usize,
    },
    ReservedAuxvEntry {
        key: u64,
        index: usize,
    },
    AddressUnderflow {
        address: u64,
        bytes: u64,
    },
    StackImageTooLarge {
        bytes: u64,
    },
    Memory(MemoryError),
}

impl fmt::Display for RiscvSeStartupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InteriorNul { field, index } => {
                write!(formatter, "RISC-V SE {field} {index} contains a nul byte")
            }
            Self::ReservedAuxvEntry { key, index } => write!(
                formatter,
                "RISC-V SE auxv entry {index} uses reserved default key {key}"
            ),
            Self::AddressUnderflow { address, bytes } => write!(
                formatter,
                "RISC-V SE stack address {address:#x} cannot reserve {bytes:#x} bytes"
            ),
            Self::StackImageTooLarge { bytes } => write!(
                formatter,
                "RISC-V SE startup stack image has {bytes:#x} bytes"
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvSeStartupError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for RiscvSeStartupStringField {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Argument => write!(formatter, "argument"),
            Self::Environment => write!(formatter, "environment"),
        }
    }
}

fn validate_strings(
    field: RiscvSeStartupStringField,
    strings: &[Vec<u8>],
) -> Result<(), RiscvSeStartupError> {
    for (index, string) in strings.iter().enumerate() {
        if string.contains(&0) {
            return Err(RiscvSeStartupError::InteriorNul { field, index });
        }
    }
    Ok(())
}

fn validate_auxv_defaults(auxv: &[RiscvSeAuxvEntry]) -> Result<(), RiscvSeStartupError> {
    for (index, entry) in auxv.iter().copied().enumerate() {
        if matches!(
            entry.key(),
            RISCV_LINUX_AT_NULL
                | RISCV_LINUX_AT_PAGESZ
                | RISCV_LINUX_AT_SECURE
                | RISCV_LINUX_AT_RANDOM
        ) {
            return Err(RiscvSeStartupError::ReservedAuxvEntry {
                key: entry.key(),
                index,
            });
        }
    }
    Ok(())
}

fn nul_terminated_len(bytes: &[u8]) -> Result<u64, RiscvSeStartupError> {
    let len = bytes
        .len()
        .checked_add(1)
        .ok_or(RiscvSeStartupError::StackImageTooLarge { bytes: u64::MAX })?;
    u64::try_from(len).map_err(|_| RiscvSeStartupError::StackImageTooLarge { bytes: u64::MAX })
}

fn frame_bytes(
    argv_pointers: &[Address],
    envp_pointers: &[Address],
    auxv: &[RiscvSeAuxvEntry],
) -> Result<u64, RiscvSeStartupError> {
    let words = 1usize
        .checked_add(argv_pointers.len())
        .and_then(|words| words.checked_add(envp_pointers.len()))
        .and_then(|words| words.checked_add(auxv.len().checked_mul(2)?))
        .ok_or(RiscvSeStartupError::StackImageTooLarge { bytes: u64::MAX })?;
    let words = u64::try_from(words)
        .map_err(|_| RiscvSeStartupError::StackImageTooLarge { bytes: u64::MAX })?;
    words
        .checked_mul(RISCV_SE_WORD_BYTES)
        .ok_or(RiscvSeStartupError::StackImageTooLarge { bytes: u64::MAX })
}

fn subtract_stack_bytes(address: u64, bytes: u64) -> Result<u64, RiscvSeStartupError> {
    address
        .checked_sub(bytes)
        .ok_or(RiscvSeStartupError::AddressUnderflow { address, bytes })
}

const fn align_down(value: u64, align: u64) -> u64 {
    value & !(align - 1)
}

fn stack_range(start: u64, end: u64) -> Result<AddressRange, RiscvSeStartupError> {
    let bytes = end
        .checked_sub(start)
        .ok_or(RiscvSeStartupError::AddressUnderflow {
            address: end,
            bytes: start,
        })?;
    let size = AccessSize::new(bytes).map_err(RiscvSeStartupError::Memory)?;
    AddressRange::new(Address::new(start), size).map_err(RiscvSeStartupError::Memory)
}

fn write_stack_u64(base: Address, data: &mut [u8], cursor: &mut u64, value: u64) {
    write_stack_bytes(base, data, Address::new(*cursor), &value.to_le_bytes());
    *cursor += RISCV_SE_WORD_BYTES;
}

fn write_stack_c_string(base: Address, data: &mut [u8], address: Address, bytes: &[u8]) {
    write_stack_bytes(base, data, address, bytes);
    write_stack_bytes(
        base,
        data,
        Address::new(address.get() + bytes.len() as u64),
        &[0],
    );
}

fn write_stack_bytes(base: Address, data: &mut [u8], address: Address, bytes: &[u8]) {
    let offset = usize::try_from(address.get() - base.get()).expect("stack offset fits usize");
    data[offset..offset + bytes.len()].copy_from_slice(bytes);
}
