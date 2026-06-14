use rem6_kernel::{PartitionId, Tick};

use crate::{GpuError, GpuIsaProgram};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuDeviceId(u32);

impl GpuDeviceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuKernelId(u64);

impl GpuKernelId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuWorkgroupId(u32);

impl GpuWorkgroupId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuDmaId(u64);

impl GpuDmaId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuWaitForMarker {
    offset: usize,
}

impl GpuWaitForMarker {
    pub(crate) const fn new(offset: usize) -> Self {
        Self { offset }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuComputeConfig {
    device: GpuDeviceId,
    partition: PartitionId,
    compute_units: u32,
    wave_slots_per_compute_unit: u32,
}

impl GpuComputeConfig {
    pub fn new(
        device: GpuDeviceId,
        partition: PartitionId,
        compute_units: u32,
        wave_slots_per_compute_unit: u32,
    ) -> Result<Self, GpuError> {
        if compute_units == 0 {
            return Err(GpuError::ZeroComputeUnits { device });
        }
        if wave_slots_per_compute_unit == 0 {
            return Err(GpuError::ZeroWaveSlots { device });
        }

        Ok(Self {
            device,
            partition,
            compute_units,
            wave_slots_per_compute_unit,
        })
    }

    pub const fn device(&self) -> GpuDeviceId {
        self.device
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn compute_units(&self) -> u32 {
        self.compute_units
    }

    pub const fn wave_slots_per_compute_unit(&self) -> u32 {
        self.wave_slots_per_compute_unit
    }

    pub(crate) fn slot_count(&self) -> usize {
        (self.compute_units as usize)
            .checked_mul(self.wave_slots_per_compute_unit as usize)
            .expect("GPU slot count fits usize")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuKernelLaunch {
    kernel: GpuKernelId,
    workgroups: u32,
    workgroup_latency: Tick,
    isa_program: GpuIsaProgram,
}

impl GpuKernelLaunch {
    pub fn new(
        kernel: GpuKernelId,
        workgroups: u32,
        workgroup_latency: Tick,
    ) -> Result<Self, GpuError> {
        if workgroups == 0 {
            return Err(GpuError::ZeroWorkgroups { kernel });
        }
        if workgroup_latency == 0 {
            return Err(GpuError::ZeroWorkgroupLatency { kernel });
        }

        Ok(Self {
            kernel,
            workgroups,
            workgroup_latency,
            isa_program: GpuIsaProgram::empty(),
        })
    }

    pub fn with_isa_program(mut self, isa_program: GpuIsaProgram) -> Self {
        self.isa_program = isa_program;
        self
    }

    pub const fn kernel(&self) -> GpuKernelId {
        self.kernel
    }

    pub const fn workgroups(&self) -> u32 {
        self.workgroups
    }

    pub const fn workgroup_latency(&self) -> Tick {
        self.workgroup_latency
    }

    pub fn isa_program(&self) -> &GpuIsaProgram {
        &self.isa_program
    }
}
