use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

use crate::{ExecutionMode, ExecutionModeTarget};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostAssistedArchitecture {
    X86_64,
    AArch64,
    Riscv64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostAssistedSimulationMode {
    FullSystem,
    SyscallEmulation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostAssistedMemoryMode {
    Atomic,
    Timing,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostAssistedPendingService {
    None,
    Mmio,
    PortIo,
    Hypercall,
    Halt,
    Mwait,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostAssistedStateComponent {
    GeneralRegisters,
    SpecialRegisters,
    FloatingPointState,
    DebugRegisters,
    ModelSpecificRegisters,
    VirtualCpuEvents,
    MemoryMappings,
    PendingWaitRequest,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostAssistedRegisterSpace {
    X86Msr,
    X86SpecialRegister,
    ArmOneReg,
    RiscvCsr,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct HostAssistedRegisterId {
    space: HostAssistedRegisterSpace,
    index: u64,
}

impl HostAssistedRegisterId {
    pub const fn new(space: HostAssistedRegisterSpace, index: u64) -> Self {
        Self { space, index }
    }

    pub const fn space(self) -> HostAssistedRegisterSpace {
        self.space
    }

    pub const fn index(self) -> u64 {
        self.index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostAssistedSwitchAction {
    QuiesceHostCpu,
    ValidateNoPendingHostService,
    CaptureState(HostAssistedStateComponent),
    InstallTargetState(ExecutionMode),
    ResumeTargetCpu,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAssistedSwitchRequest {
    tick: Tick,
    target: ExecutionModeTarget,
    architecture: HostAssistedArchitecture,
    simulation_mode: HostAssistedSimulationMode,
    target_mode: ExecutionMode,
    host_memory_mode: HostAssistedMemoryMode,
    target_memory_mode: HostAssistedMemoryMode,
    state_components: BTreeSet<HostAssistedStateComponent>,
    unsupported_registers: Vec<HostAssistedRegisterId>,
    pending_service: HostAssistedPendingService,
}

impl HostAssistedSwitchRequest {
    pub fn new(
        tick: Tick,
        target: ExecutionModeTarget,
        architecture: HostAssistedArchitecture,
        simulation_mode: HostAssistedSimulationMode,
        target_mode: ExecutionMode,
    ) -> Self {
        Self {
            tick,
            target,
            architecture,
            simulation_mode,
            target_mode,
            host_memory_mode: HostAssistedMemoryMode::Timing,
            target_memory_mode: HostAssistedMemoryMode::Timing,
            state_components: BTreeSet::new(),
            unsupported_registers: Vec::new(),
            pending_service: HostAssistedPendingService::None,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn target(&self) -> &ExecutionModeTarget {
        &self.target
    }

    pub const fn architecture(&self) -> HostAssistedArchitecture {
        self.architecture
    }

    pub const fn simulation_mode(&self) -> HostAssistedSimulationMode {
        self.simulation_mode
    }

    pub const fn target_mode(&self) -> ExecutionMode {
        self.target_mode
    }

    pub const fn host_memory_mode(&self) -> HostAssistedMemoryMode {
        self.host_memory_mode
    }

    pub const fn target_memory_mode(&self) -> HostAssistedMemoryMode {
        self.target_memory_mode
    }

    pub fn state_components(&self) -> &BTreeSet<HostAssistedStateComponent> {
        &self.state_components
    }

    pub fn unsupported_registers(&self) -> &[HostAssistedRegisterId] {
        &self.unsupported_registers
    }

    pub const fn pending_service(&self) -> HostAssistedPendingService {
        self.pending_service
    }

    pub const fn with_host_memory_mode(mut self, mode: HostAssistedMemoryMode) -> Self {
        self.host_memory_mode = mode;
        self
    }

    pub const fn with_target_memory_mode(mut self, mode: HostAssistedMemoryMode) -> Self {
        self.target_memory_mode = mode;
        self
    }

    pub fn with_state_component(mut self, component: HostAssistedStateComponent) -> Self {
        self.state_components.insert(component);
        self
    }

    pub fn with_unsupported_register(mut self, register: HostAssistedRegisterId) -> Self {
        self.unsupported_registers.push(register);
        self.unsupported_registers.sort();
        self.unsupported_registers.dedup();
        self
    }

    pub const fn with_pending_service(mut self, service: HostAssistedPendingService) -> Self {
        self.pending_service = service;
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostAssistedSwitchPlan {
    tick: Tick,
    target: ExecutionModeTarget,
    target_mode: ExecutionMode,
    actions: Vec<HostAssistedSwitchAction>,
}

impl HostAssistedSwitchPlan {
    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn target(&self) -> &ExecutionModeTarget {
        &self.target
    }

    pub const fn target_mode(&self) -> ExecutionMode {
        self.target_mode
    }

    pub fn actions(&self) -> &[HostAssistedSwitchAction] {
        &self.actions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostAssistedSwitchError {
    PendingHostService {
        target: ExecutionModeTarget,
        service: HostAssistedPendingService,
    },
    UnsupportedHostRegisters {
        target: ExecutionModeTarget,
        registers: Vec<HostAssistedRegisterId>,
    },
    MemoryModeMismatch {
        target: ExecutionModeTarget,
        host: HostAssistedMemoryMode,
        target_mode: HostAssistedMemoryMode,
    },
    MissingHostStateComponents {
        target: ExecutionModeTarget,
        missing: Vec<HostAssistedStateComponent>,
    },
}

impl fmt::Display for HostAssistedSwitchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PendingHostService { target, service } => write!(
                formatter,
                "host-assisted target {} still has pending service {service:?}",
                target.as_str()
            ),
            Self::UnsupportedHostRegisters { target, registers } => write!(
                formatter,
                "host-assisted target {} has unsupported host registers {registers:?}",
                target.as_str()
            ),
            Self::MemoryModeMismatch {
                target,
                host,
                target_mode,
            } => write!(
                formatter,
                "host-assisted target {} memory mode {host:?} does not match target {target_mode:?}",
                target.as_str()
            ),
            Self::MissingHostStateComponents { target, missing } => write!(
                formatter,
                "host-assisted target {} is missing host state components {missing:?}",
                target.as_str()
            ),
        }
    }
}

impl Error for HostAssistedSwitchError {}

pub struct HostAssistedSwitchPlanner;

impl HostAssistedSwitchPlanner {
    pub fn plan(
        request: &HostAssistedSwitchRequest,
    ) -> Result<HostAssistedSwitchPlan, HostAssistedSwitchError> {
        if request.pending_service() != HostAssistedPendingService::None {
            return Err(HostAssistedSwitchError::PendingHostService {
                target: request.target().clone(),
                service: request.pending_service(),
            });
        }

        if !request.unsupported_registers().is_empty() {
            return Err(HostAssistedSwitchError::UnsupportedHostRegisters {
                target: request.target().clone(),
                registers: request.unsupported_registers().to_vec(),
            });
        }

        if request.host_memory_mode() != request.target_memory_mode() {
            return Err(HostAssistedSwitchError::MemoryModeMismatch {
                target: request.target().clone(),
                host: request.host_memory_mode(),
                target_mode: request.target_memory_mode(),
            });
        }

        let required_components =
            required_state_components(request.architecture(), request.simulation_mode());
        let missing = required_components
            .iter()
            .copied()
            .filter(|component| !request.state_components().contains(component))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            return Err(HostAssistedSwitchError::MissingHostStateComponents {
                target: request.target().clone(),
                missing,
            });
        }

        let mut actions = Vec::with_capacity(required_components.len() + 4);
        actions.push(HostAssistedSwitchAction::QuiesceHostCpu);
        actions.push(HostAssistedSwitchAction::ValidateNoPendingHostService);
        actions.extend(
            required_components
                .iter()
                .copied()
                .map(HostAssistedSwitchAction::CaptureState),
        );
        actions.push(HostAssistedSwitchAction::InstallTargetState(
            request.target_mode(),
        ));
        actions.push(HostAssistedSwitchAction::ResumeTargetCpu);

        Ok(HostAssistedSwitchPlan {
            tick: request.tick(),
            target: request.target().clone(),
            target_mode: request.target_mode(),
            actions,
        })
    }
}

fn required_state_components(
    architecture: HostAssistedArchitecture,
    simulation_mode: HostAssistedSimulationMode,
) -> &'static [HostAssistedStateComponent] {
    match (architecture, simulation_mode) {
        (HostAssistedArchitecture::X86_64, HostAssistedSimulationMode::FullSystem) => &[
            HostAssistedStateComponent::GeneralRegisters,
            HostAssistedStateComponent::SpecialRegisters,
            HostAssistedStateComponent::FloatingPointState,
            HostAssistedStateComponent::DebugRegisters,
            HostAssistedStateComponent::ModelSpecificRegisters,
            HostAssistedStateComponent::VirtualCpuEvents,
            HostAssistedStateComponent::MemoryMappings,
            HostAssistedStateComponent::PendingWaitRequest,
        ],
        (HostAssistedArchitecture::X86_64, HostAssistedSimulationMode::SyscallEmulation) => &[
            HostAssistedStateComponent::GeneralRegisters,
            HostAssistedStateComponent::SpecialRegisters,
            HostAssistedStateComponent::FloatingPointState,
            HostAssistedStateComponent::ModelSpecificRegisters,
            HostAssistedStateComponent::PendingWaitRequest,
        ],
        (HostAssistedArchitecture::AArch64, HostAssistedSimulationMode::FullSystem) => &[
            HostAssistedStateComponent::GeneralRegisters,
            HostAssistedStateComponent::SpecialRegisters,
            HostAssistedStateComponent::FloatingPointState,
            HostAssistedStateComponent::VirtualCpuEvents,
            HostAssistedStateComponent::MemoryMappings,
            HostAssistedStateComponent::PendingWaitRequest,
        ],
        (HostAssistedArchitecture::AArch64, HostAssistedSimulationMode::SyscallEmulation) => &[
            HostAssistedStateComponent::GeneralRegisters,
            HostAssistedStateComponent::SpecialRegisters,
            HostAssistedStateComponent::FloatingPointState,
            HostAssistedStateComponent::PendingWaitRequest,
        ],
        (HostAssistedArchitecture::Riscv64, HostAssistedSimulationMode::FullSystem) => &[
            HostAssistedStateComponent::GeneralRegisters,
            HostAssistedStateComponent::SpecialRegisters,
            HostAssistedStateComponent::MemoryMappings,
            HostAssistedStateComponent::PendingWaitRequest,
        ],
        (HostAssistedArchitecture::Riscv64, HostAssistedSimulationMode::SyscallEmulation) => &[
            HostAssistedStateComponent::GeneralRegisters,
            HostAssistedStateComponent::SpecialRegisters,
            HostAssistedStateComponent::PendingWaitRequest,
        ],
    }
}
