use rem6_system::{
    ExecutionMode, ExecutionModeTarget, HostAssistedArchitecture, HostAssistedMemoryMode,
    HostAssistedPendingService, HostAssistedRegisterId, HostAssistedRegisterSpace,
    HostAssistedSimulationMode, HostAssistedStateComponent, HostAssistedSwitchAction,
    HostAssistedSwitchError, HostAssistedSwitchPlanner, HostAssistedSwitchRequest,
};

#[test]
fn host_assisted_switch_plan_captures_complete_state_before_detailed_takeover() {
    let target = ExecutionModeTarget::new("cpu0");
    let request = complete_x86_switch_request(target.clone());

    let plan = HostAssistedSwitchPlanner::plan(&request).unwrap();

    assert_eq!(plan.tick(), 111);
    assert_eq!(plan.target(), &target);
    assert_eq!(plan.target_mode(), ExecutionMode::Detailed);
    assert_eq!(
        plan.actions(),
        &[
            HostAssistedSwitchAction::QuiesceHostCpu,
            HostAssistedSwitchAction::ValidateNoPendingHostService,
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::GeneralRegisters),
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::SpecialRegisters),
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::FloatingPointState),
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::DebugRegisters),
            HostAssistedSwitchAction::CaptureState(
                HostAssistedStateComponent::ModelSpecificRegisters,
            ),
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::VirtualCpuEvents),
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::MemoryMappings),
            HostAssistedSwitchAction::CaptureState(HostAssistedStateComponent::PendingWaitRequest),
            HostAssistedSwitchAction::InstallTargetState(ExecutionMode::Detailed),
            HostAssistedSwitchAction::ResumeTargetCpu,
        ]
    );
}

#[test]
fn host_assisted_switch_plan_rejects_pending_mwait_before_target_takeover() {
    let target = ExecutionModeTarget::new("cpu0");
    let request = complete_x86_switch_request(target.clone())
        .with_pending_service(HostAssistedPendingService::Mwait);

    assert_eq!(
        HostAssistedSwitchPlanner::plan(&request).unwrap_err(),
        HostAssistedSwitchError::PendingHostService {
            target,
            service: HostAssistedPendingService::Mwait,
        }
    );
}

#[test]
fn host_assisted_switch_plan_rejects_incomplete_architectural_state() {
    let target = ExecutionModeTarget::new("cpu0");
    let request = HostAssistedSwitchRequest::new(
        111,
        target.clone(),
        HostAssistedArchitecture::X86_64,
        HostAssistedSimulationMode::FullSystem,
        ExecutionMode::Detailed,
    )
    .with_host_memory_mode(HostAssistedMemoryMode::Timing)
    .with_target_memory_mode(HostAssistedMemoryMode::Timing)
    .with_state_component(HostAssistedStateComponent::GeneralRegisters);

    assert_eq!(
        HostAssistedSwitchPlanner::plan(&request).unwrap_err(),
        HostAssistedSwitchError::MissingHostStateComponents {
            target,
            missing: vec![
                HostAssistedStateComponent::SpecialRegisters,
                HostAssistedStateComponent::FloatingPointState,
                HostAssistedStateComponent::DebugRegisters,
                HostAssistedStateComponent::ModelSpecificRegisters,
                HostAssistedStateComponent::VirtualCpuEvents,
                HostAssistedStateComponent::MemoryMappings,
                HostAssistedStateComponent::PendingWaitRequest,
            ],
        }
    );
}

#[test]
fn host_assisted_switch_plan_rejects_unsupported_host_registers() {
    let target = ExecutionModeTarget::new("cpu0");
    let unsupported_msr =
        HostAssistedRegisterId::new(HostAssistedRegisterSpace::X86Msr, 0x40000010);
    let request =
        complete_x86_switch_request(target.clone()).with_unsupported_register(unsupported_msr);

    assert_eq!(
        HostAssistedSwitchPlanner::plan(&request).unwrap_err(),
        HostAssistedSwitchError::UnsupportedHostRegisters {
            target,
            registers: vec![unsupported_msr],
        }
    );
}

#[test]
fn host_assisted_switch_plan_rejects_memory_mode_mismatch() {
    let target = ExecutionModeTarget::new("cpu0");
    let request = complete_x86_switch_request(target.clone())
        .with_host_memory_mode(HostAssistedMemoryMode::Atomic)
        .with_target_memory_mode(HostAssistedMemoryMode::Timing);

    assert_eq!(
        HostAssistedSwitchPlanner::plan(&request).unwrap_err(),
        HostAssistedSwitchError::MemoryModeMismatch {
            target,
            host: HostAssistedMemoryMode::Atomic,
            target_mode: HostAssistedMemoryMode::Timing,
        }
    );
}

fn complete_x86_switch_request(target: ExecutionModeTarget) -> HostAssistedSwitchRequest {
    HostAssistedSwitchRequest::new(
        111,
        target,
        HostAssistedArchitecture::X86_64,
        HostAssistedSimulationMode::FullSystem,
        ExecutionMode::Detailed,
    )
    .with_host_memory_mode(HostAssistedMemoryMode::Timing)
    .with_target_memory_mode(HostAssistedMemoryMode::Timing)
    .with_state_component(HostAssistedStateComponent::GeneralRegisters)
    .with_state_component(HostAssistedStateComponent::SpecialRegisters)
    .with_state_component(HostAssistedStateComponent::FloatingPointState)
    .with_state_component(HostAssistedStateComponent::DebugRegisters)
    .with_state_component(HostAssistedStateComponent::ModelSpecificRegisters)
    .with_state_component(HostAssistedStateComponent::VirtualCpuEvents)
    .with_state_component(HostAssistedStateComponent::MemoryMappings)
    .with_state_component(HostAssistedStateComponent::PendingWaitRequest)
}
