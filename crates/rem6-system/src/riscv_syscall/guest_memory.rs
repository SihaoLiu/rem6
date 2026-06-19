use std::{fmt, sync::Arc};

type RiscvGuestMemoryReadFn = dyn Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static;

#[derive(Clone)]
pub struct RiscvGuestMemoryReader {
    read: Arc<RiscvGuestMemoryReadFn>,
}

impl RiscvGuestMemoryReader {
    pub fn new<F>(read: F) -> Self
    where
        F: Fn(u64, usize) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        Self {
            read: Arc::new(read),
        }
    }

    pub(crate) fn read(&self, address: u64, bytes: usize) -> Option<Vec<u8>> {
        (self.read)(address, bytes)
    }
}

impl fmt::Debug for RiscvGuestMemoryReader {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiscvGuestMemoryReader")
            .finish_non_exhaustive()
    }
}

type RiscvGuestMemoryWriteFn = dyn Fn(u64, &[u8]) -> bool + Send + Sync + 'static;
type RiscvGuestMemoryWriteProbeFn = dyn Fn(u64, usize) -> bool + Send + Sync + 'static;
type RiscvGuestMemoryMapFn =
    dyn Fn(RiscvGuestMemoryMapRequest) -> RiscvGuestMemoryMapResult + Send + Sync + 'static;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvGuestMemoryMapRequest {
    address: u64,
    bytes: u64,
    replace_existing: bool,
}

impl RiscvGuestMemoryMapRequest {
    pub const fn new(address: u64, bytes: u64, replace_existing: bool) -> Self {
        Self {
            address,
            bytes,
            replace_existing,
        }
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub const fn replace_existing(self) -> bool {
        self.replace_existing
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvGuestMemoryMapResult {
    Mapped,
    Overlap,
    Failed,
}

#[derive(Clone)]
pub struct RiscvGuestMemoryWriter {
    write: Arc<RiscvGuestMemoryWriteFn>,
    write_probe: Arc<RiscvGuestMemoryWriteProbeFn>,
    map_region: Option<Arc<RiscvGuestMemoryMapFn>>,
}

impl RiscvGuestMemoryWriter {
    pub fn new<F>(write: F) -> Self
    where
        F: Fn(u64, &[u8]) -> bool + Send + Sync + 'static,
    {
        let write: Arc<RiscvGuestMemoryWriteFn> = Arc::new(write);
        let write_probe = Arc::clone(&write);
        Self {
            write,
            write_probe: Arc::new(move |address, bytes| {
                default_write_probe(write_probe.as_ref(), address, bytes)
            }),
            map_region: None,
        }
    }

    pub fn with_write_probe<F>(mut self, write_probe: F) -> Self
    where
        F: Fn(u64, usize) -> bool + Send + Sync + 'static,
    {
        self.write_probe = Arc::new(write_probe);
        self
    }

    pub fn with_region_mapper<F>(mut self, map_region: F) -> Self
    where
        F: Fn(u64, u64) -> bool + Send + Sync + 'static,
    {
        self.map_region = Some(Arc::new(move |request| {
            if map_region(request.address(), request.bytes()) {
                RiscvGuestMemoryMapResult::Mapped
            } else {
                RiscvGuestMemoryMapResult::Failed
            }
        }));
        self
    }

    pub fn with_region_map_handler<F>(mut self, map_region: F) -> Self
    where
        F: Fn(RiscvGuestMemoryMapRequest) -> RiscvGuestMemoryMapResult + Send + Sync + 'static,
    {
        self.map_region = Some(Arc::new(map_region));
        self
    }

    pub(in crate::riscv_syscall) fn write(&self, address: u64, bytes: &[u8]) -> bool {
        (self.write)(address, bytes)
    }

    pub(in crate::riscv_syscall) fn can_write(&self, address: u64, bytes: usize) -> bool {
        (self.write_probe)(address, bytes)
    }

    pub(in crate::riscv_syscall) fn map_region(
        &self,
        address: u64,
        bytes: u64,
        replace_existing: bool,
    ) -> RiscvGuestMemoryMapResult {
        match &self.map_region {
            Some(map_region) => map_region(RiscvGuestMemoryMapRequest::new(
                address,
                bytes,
                replace_existing,
            )),
            None => RiscvGuestMemoryMapResult::Mapped,
        }
    }
}

fn default_write_probe(write: &RiscvGuestMemoryWriteFn, address: u64, bytes: usize) -> bool {
    if bytes == 0 {
        return true;
    }
    if !write(address, &[]) {
        return false;
    }
    let Some(last_byte) = u64::try_from(bytes)
        .ok()
        .and_then(|bytes| bytes.checked_sub(1))
        .and_then(|last_offset| address.checked_add(last_offset))
    else {
        return false;
    };
    last_byte == address || write(last_byte, &[])
}

impl fmt::Debug for RiscvGuestMemoryWriter {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RiscvGuestMemoryWriter")
            .finish_non_exhaustive()
    }
}
