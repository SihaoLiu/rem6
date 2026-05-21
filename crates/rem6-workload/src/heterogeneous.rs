use rem6_kernel::Tick;
use rem6_memory::Address;

use crate::{WorkloadError, WorkloadRouteId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadGpuDevice {
    device: u32,
    partition: u32,
    compute_units: u32,
    wave_slots_per_compute_unit: u32,
    command_route: WorkloadRouteId,
}

impl WorkloadGpuDevice {
    pub fn new(
        device: u32,
        partition: u32,
        compute_units: u32,
        wave_slots_per_compute_unit: u32,
        command_route: WorkloadRouteId,
    ) -> Result<Self, WorkloadError> {
        if compute_units == 0 {
            return Err(WorkloadError::ZeroGpuComputeUnits { device });
        }
        if wave_slots_per_compute_unit == 0 {
            return Err(WorkloadError::ZeroGpuWaveSlots { device });
        }

        Ok(Self {
            device,
            partition,
            compute_units,
            wave_slots_per_compute_unit,
            command_route,
        })
    }

    pub const fn device(&self) -> u32 {
        self.device
    }

    pub const fn partition(&self) -> u32 {
        self.partition
    }

    pub const fn compute_units(&self) -> u32 {
        self.compute_units
    }

    pub const fn wave_slots_per_compute_unit(&self) -> u32 {
        self.wave_slots_per_compute_unit
    }

    pub const fn command_route(&self) -> &WorkloadRouteId {
        &self.command_route
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadGpuKernelLaunch {
    device: u32,
    kernel: u64,
    workgroups: u32,
    workgroup_latency: Tick,
}

impl WorkloadGpuKernelLaunch {
    pub const fn new(
        device: u32,
        kernel: u64,
        workgroups: u32,
        workgroup_latency: Tick,
    ) -> Result<Self, WorkloadError> {
        if workgroups == 0 {
            return Err(WorkloadError::ZeroGpuKernelWorkgroups { device, kernel });
        }
        if workgroup_latency == 0 {
            return Err(WorkloadError::ZeroGpuKernelLatency { device, kernel });
        }

        Ok(Self {
            device,
            kernel,
            workgroups,
            workgroup_latency,
        })
    }

    pub const fn device(&self) -> u32 {
        self.device
    }

    pub const fn kernel(&self) -> u64 {
        self.kernel
    }

    pub const fn workgroups(&self) -> u32 {
        self.workgroups
    }

    pub const fn workgroup_latency(&self) -> Tick {
        self.workgroup_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadGpuDmaCopy {
    device: u32,
    transfer: u64,
    route: WorkloadRouteId,
    agent: u32,
    source: Address,
    destination: Address,
    bytes: u64,
}

impl WorkloadGpuDmaCopy {
    pub fn new(
        device: u32,
        transfer: u64,
        route: WorkloadRouteId,
        agent: u32,
        source: Address,
        destination: Address,
        bytes: u64,
    ) -> Result<Self, WorkloadError> {
        if bytes == 0 {
            return Err(WorkloadError::ZeroGpuDmaBytes { device, transfer });
        }

        Ok(Self {
            device,
            transfer,
            route,
            agent,
            source,
            destination,
            bytes,
        })
    }

    pub const fn device(&self) -> u32 {
        self.device
    }

    pub const fn transfer(&self) -> u64 {
        self.transfer
    }

    pub const fn route(&self) -> &WorkloadRouteId {
        &self.route
    }

    pub const fn agent(&self) -> u32 {
        self.agent
    }

    pub const fn source(&self) -> Address {
        self.source
    }

    pub const fn destination(&self) -> Address {
        self.destination
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadAcceleratorDevice {
    engine: u32,
    partition: u32,
    lanes: u32,
    command_route: WorkloadRouteId,
}

impl WorkloadAcceleratorDevice {
    pub fn new(
        engine: u32,
        partition: u32,
        lanes: u32,
        command_route: WorkloadRouteId,
    ) -> Result<Self, WorkloadError> {
        if lanes == 0 {
            return Err(WorkloadError::ZeroAcceleratorLanes { engine });
        }

        Ok(Self {
            engine,
            partition,
            lanes,
            command_route,
        })
    }

    pub const fn engine(&self) -> u32 {
        self.engine
    }

    pub const fn partition(&self) -> u32 {
        self.partition
    }

    pub const fn lanes(&self) -> u32 {
        self.lanes
    }

    pub const fn command_route(&self) -> &WorkloadRouteId {
        &self.command_route
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadAcceleratorCommandKind {
    GpuKernel { workgroups: u32 },
    NpuInference { tiles: u32 },
    DmaCopy { bytes: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadAcceleratorCommand {
    engine: u32,
    command: u64,
    kind: WorkloadAcceleratorCommandKind,
    execution_latency: Tick,
}

impl WorkloadAcceleratorCommand {
    pub const fn new(
        engine: u32,
        command: u64,
        kind: WorkloadAcceleratorCommandKind,
        execution_latency: Tick,
    ) -> Result<Self, WorkloadError> {
        if execution_latency == 0 {
            return Err(WorkloadError::ZeroAcceleratorExecutionLatency { engine, command });
        }
        match &kind {
            WorkloadAcceleratorCommandKind::GpuKernel { workgroups: 0 } => {
                return Err(WorkloadError::ZeroAcceleratorGpuWorkgroups { engine, command });
            }
            WorkloadAcceleratorCommandKind::NpuInference { tiles: 0 } => {
                return Err(WorkloadError::ZeroAcceleratorNpuTiles { engine, command });
            }
            WorkloadAcceleratorCommandKind::DmaCopy { bytes: 0 } => {
                return Err(WorkloadError::ZeroAcceleratorDmaBytes { engine, command });
            }
            _ => {}
        }

        Ok(Self {
            engine,
            command,
            kind,
            execution_latency,
        })
    }

    pub const fn engine(&self) -> u32 {
        self.engine
    }

    pub const fn command(&self) -> u64 {
        self.command
    }

    pub const fn kind(&self) -> &WorkloadAcceleratorCommandKind {
        &self.kind
    }

    pub const fn execution_latency(&self) -> Tick {
        self.execution_latency
    }
}
