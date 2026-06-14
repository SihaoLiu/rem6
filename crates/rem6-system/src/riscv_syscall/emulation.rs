use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::RiscvCore;
use rem6_kernel::Tick;

use crate::RiscvSystemRunDriver;

use super::*;

#[derive(Clone, Debug)]
pub struct RiscvSyscallEmulation {
    table: RiscvSyscallTable,
    state: Arc<Mutex<RiscvSyscallState>>,
    guest_memory_reader: Option<RiscvGuestMemoryReader>,
    guest_memory_writer: Option<RiscvGuestMemoryWriter>,
}

impl RiscvSyscallEmulation {
    pub fn new(table: RiscvSyscallTable, state: RiscvSyscallState) -> Self {
        Self {
            table,
            state: Arc::new(Mutex::new(state)),
            guest_memory_reader: None,
            guest_memory_writer: None,
        }
    }

    pub fn linux_user() -> Self {
        Self::new(RiscvSyscallTable::new(), RiscvSyscallState::new(0))
    }

    pub fn linux_user_for_boot_image(image: &BootImage) -> Self {
        Self::try_linux_user_for_boot_image(image)
            .expect("RISC-V SE boot image program break is representable")
    }

    pub fn try_linux_user_for_boot_image(
        image: &BootImage,
    ) -> Result<Self, RiscvSyscallImageLayoutError> {
        Ok(Self::new(
            RiscvSyscallTable::new(),
            RiscvSyscallState::new(riscv_program_break_for_boot_image(image)?),
        ))
    }

    fn with_boot_image_program_break(
        self,
        image: &BootImage,
    ) -> Result<Self, RiscvSyscallImageLayoutError> {
        let program_break = riscv_program_break_for_boot_image(image)?;
        {
            let mut state = self.state.lock().expect("RISC-V syscall state lock");
            state.initial_program_break = program_break;
            state.set_program_break(program_break);
            state.set_program_break_backing_end(program_break);
        }
        Ok(self)
    }

    pub const fn table(&self) -> RiscvSyscallTable {
        self.table
    }

    pub fn with_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        self.guest_memory_reader = Some(RiscvGuestMemoryReader::new(read));
        self
    }

    pub fn with_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        self.guest_memory_writer = Some(RiscvGuestMemoryWriter::new(write));
        self
    }

    pub fn with_mapped_guest_memory_writer<W, M>(mut self, write: W, map_region: M) -> Self
    where
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        self.guest_memory_writer =
            Some(RiscvGuestMemoryWriter::new(write).with_region_mapper(map_region));
        self
    }

    pub fn with_guest_memory_map_handler<W, M>(mut self, write: W, map_region: M) -> Self
    where
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(RiscvGuestMemoryMapRequest) -> RiscvGuestMemoryMapResult + Send + Sync + 'static,
    {
        self.guest_memory_writer =
            Some(RiscvGuestMemoryWriter::new(write).with_region_map_handler(map_region));
        self
    }

    pub fn state(&self) -> RiscvSyscallState {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .clone()
    }

    pub(super) fn with_state_mut<R>(&self, f: impl FnOnce(&mut RiscvSyscallState) -> R) -> R {
        let mut state = self.state.lock().expect("RISC-V syscall state lock");
        f(&mut state)
    }

    pub fn push_stdin_bytes(&self, bytes: &[u8]) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .push_stdin_bytes(bytes);
    }

    pub fn register_guest_path(&self, path: impl AsRef<[u8]>) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .register_guest_path(path);
    }

    pub fn register_guest_file(&self, path: impl AsRef<[u8]>, contents: impl AsRef<[u8]>) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .register_guest_file(path, contents);
    }

    pub fn register_guest_symlink(&self, path: impl AsRef<[u8]>, target: impl AsRef<[u8]>) {
        self.state
            .lock()
            .expect("RISC-V syscall state lock")
            .register_guest_symlink(path, target);
    }

    pub fn handle_pending_core_trap(
        &self,
        core: &RiscvCore,
        tick: Tick,
    ) -> Option<RiscvSyscallOutcome> {
        let request = RiscvSyscallRequest::from_pending_core_trap(core)?;
        let mut state = self.state.lock().expect("RISC-V syscall state lock");
        self.table.handle_with_guest_memory_io_at_tick(
            request,
            &mut state,
            tick,
            self.guest_memory_reader.as_ref(),
            self.guest_memory_writer.as_ref(),
        )
    }
}

impl Default for RiscvSyscallEmulation {
    fn default() -> Self {
        Self::linux_user()
    }
}

impl RiscvSystemRunDriver {
    pub fn with_riscv_syscall_emulation(mut self) -> Self {
        self.riscv_syscall_emulation = Some(RiscvSyscallEmulation::linux_user());
        self
    }

    pub fn with_riscv_syscall_emulation_for_boot_image(self, image: &BootImage) -> Self {
        self.try_with_riscv_syscall_emulation_for_boot_image(image)
            .expect("RISC-V SE boot image program break is representable")
    }

    pub fn try_with_riscv_syscall_emulation_for_boot_image(
        mut self,
        image: &BootImage,
    ) -> Result<Self, RiscvSyscallImageLayoutError> {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_boot_image_program_break(image)?;
        self.riscv_syscall_emulation = Some(emulation);
        Ok(self)
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_reader<F>(mut self, read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_writer<F>(mut self, write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_writer(write);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_mapped_guest_memory_writer<W, M>(
        mut self,
        write: W,
        map_region: M,
    ) -> Self
    where
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_mapped_guest_memory_writer(write, map_region);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_io<R, W>(
        mut self,
        read: R,
        write: W,
    ) -> Self
    where
        R: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read)
            .with_guest_memory_writer(write);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_mapped_guest_memory_io<R, W, M>(
        mut self,
        read: R,
        write: W,
        map_region: M,
    ) -> Self
    where
        R: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read)
            .with_mapped_guest_memory_writer(write, map_region);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub fn with_riscv_syscall_emulation_and_guest_memory_io_map_handler<R, W, M>(
        mut self,
        read: R,
        write: W,
        map_region: M,
    ) -> Self
    where
        R: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
        W: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
        M: Fn(RiscvGuestMemoryMapRequest) -> RiscvGuestMemoryMapResult + Send + Sync + 'static,
    {
        let emulation = self
            .take_riscv_syscall_emulation_or_linux_user()
            .with_guest_memory_reader(read)
            .with_guest_memory_map_handler(write, map_region);
        self.riscv_syscall_emulation = Some(emulation);
        self
    }

    pub const fn riscv_syscall_emulation(&self) -> Option<&RiscvSyscallEmulation> {
        self.riscv_syscall_emulation.as_ref()
    }

    fn take_riscv_syscall_emulation_or_linux_user(&mut self) -> RiscvSyscallEmulation {
        self.riscv_syscall_emulation
            .take()
            .unwrap_or_else(RiscvSyscallEmulation::linux_user)
    }
}
