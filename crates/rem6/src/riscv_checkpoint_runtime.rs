use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::RiscvCluster;
use rem6_dram::DramMemoryController;
use rem6_system::{
    CheckpointRuntimeState, DramMemoryCheckpointBank, DramMemoryCheckpointPort,
    FabricCheckpointBank, FabricCheckpointPort, MemoryStoreCheckpointBank,
    MemoryStoreCheckpointPort, MsiBankCheckpointBank, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, SystemHostController,
};
use rem6_transport::MemoryTransport;

use crate::data_cache_runtime::CliCacheHierarchy;
use crate::runtime_memory::{CliMemoryCheckpointSource, CliMemoryRuntime};
use crate::{execute_error, Rem6CliError};

pub(crate) fn attach_cli_riscv_checkpoint_bank(
    controller: &Arc<Mutex<SystemHostController>>,
    cluster: &RiscvCluster,
) -> Result<(), Rem6CliError> {
    let ports = cluster
        .core_ids()
        .into_iter()
        .map(|cpu| {
            let component =
                CheckpointComponentId::new(format!("cpu{}", cpu.get())).map_err(execute_error)?;
            let core = cluster.core(cpu).map_err(execute_error)?;
            Ok(RiscvCoreCheckpointPort::new(component, core))
        })
        .collect::<Result<Vec<_>, Rem6CliError>>()?;
    let bank = RiscvCoreCheckpointBank::new(ports).map_err(execute_error)?;
    controller
        .lock()
        .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?
        .executor_mut()
        .attach_riscv_checkpoint_bank(bank)
        .map_err(execute_error)
}

pub(crate) fn attach_cli_memory_checkpoint_bank(
    controller: &Arc<Mutex<SystemHostController>>,
    memory: &CliMemoryRuntime,
) -> Result<(), Rem6CliError> {
    let component = CheckpointComponentId::new("memory0").map_err(execute_error)?;
    let mut controller = controller
        .lock()
        .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?;
    match memory.checkpoint_source() {
        CliMemoryCheckpointSource::Store(store) => {
            let bank =
                MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(component, store)])
                    .map_err(execute_error)?;
            controller
                .executor_mut()
                .attach_memory_checkpoint_bank(bank)
                .map_err(execute_error)
        }
        CliMemoryCheckpointSource::Dram(memory) => {
            let runtime_state = dram_runtime_checkpoint_state(&memory);
            let port =
                DramMemoryCheckpointPort::new(component, memory).with_runtime_state(runtime_state);
            let bank = DramMemoryCheckpointBank::new([port]).map_err(execute_error)?;
            controller
                .executor_mut()
                .attach_dram_memory_checkpoint_bank(bank)
                .map_err(execute_error)
        }
    }
}

fn dram_runtime_checkpoint_state(
    controller: &Arc<Mutex<DramMemoryController>>,
) -> CheckpointRuntimeState {
    let capture_controller = Arc::clone(controller);
    let validate_controller = Arc::clone(controller);
    let restore_controller = Arc::clone(controller);
    CheckpointRuntimeState::stored(
        move || {
            Ok(capture_controller
                .lock()
                .expect("DRAM checkpoint runtime lock")
                .runtime_logs())
        },
        move |snapshot| {
            validate_controller
                .lock()
                .expect("DRAM checkpoint runtime lock")
                .validate_runtime_logs(snapshot)
                .map_err(|error| error.to_string())
        },
        move |snapshot| {
            restore_controller
                .lock()
                .expect("DRAM checkpoint runtime lock")
                .restore_runtime_logs(snapshot)
                .expect("validated DRAM runtime logs");
        },
    )
}

pub(crate) fn attach_cli_fabric_checkpoint_bank(
    controller: &Arc<Mutex<SystemHostController>>,
    transport: &MemoryTransport,
) -> Result<(), Rem6CliError> {
    let Some(fabric) = transport.fabric() else {
        return Ok(());
    };
    let component = CheckpointComponentId::new("fabric0").map_err(execute_error)?;
    let runtime_state = fabric_runtime_checkpoint_state(&fabric);
    let port = FabricCheckpointPort::new(component, fabric).with_runtime_state(runtime_state);
    let bank = FabricCheckpointBank::new([port]).map_err(execute_error)?;
    controller
        .lock()
        .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?
        .executor_mut()
        .attach_fabric_checkpoint_bank(bank)
        .map_err(execute_error)
}

fn fabric_runtime_checkpoint_state(
    fabric: &Arc<Mutex<rem6_fabric::FabricModel>>,
) -> CheckpointRuntimeState {
    let capture_fabric = Arc::clone(fabric);
    let restore_fabric = Arc::clone(fabric);
    CheckpointRuntimeState::stored(
        move || {
            Ok(capture_fabric
                .lock()
                .expect("fabric checkpoint runtime lock")
                .runtime_logs())
        },
        |_| Ok(()),
        move |snapshot| {
            restore_fabric
                .lock()
                .expect("fabric checkpoint runtime lock")
                .restore_runtime_logs(snapshot);
        },
    )
}

pub(crate) fn attach_cli_cache_checkpoint_bank(
    controller: &Arc<Mutex<SystemHostController>>,
    instruction_cache: &CliCacheHierarchy,
    data_cache: &CliCacheHierarchy,
) -> Result<(), Rem6CliError> {
    if let Some(reason) = instruction_cache
        .msi_checkpoint_unsupported_reason()
        .or_else(|| data_cache.msi_checkpoint_unsupported_reason())
    {
        controller
            .lock()
            .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?
            .executor_mut()
            .reject_checkpoint_actions(reason);
        return Ok(());
    }
    let mut ports = instruction_cache.msi_checkpoint_ports("icache")?;
    ports.extend(data_cache.msi_checkpoint_ports("dcache")?);
    if ports.is_empty() {
        return Ok(());
    }
    let bank = MsiBankCheckpointBank::new(ports).map_err(execute_error)?;
    controller
        .lock()
        .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?
        .executor_mut()
        .attach_msi_bank_checkpoint_bank(bank)
        .map_err(execute_error)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rem6_checkpoint::CheckpointRegistry;
    use rem6_fabric::{
        FabricLinkId, FabricModel, FabricPacket, FabricPacketId, FabricPath, FabricPathHop,
        VirtualNetworkId,
    };

    const FABRIC_RUNTIME_STATE_CHUNK: &str = "fabric-runtime-state";

    fn route() -> FabricPath {
        FabricPath::new([
            FabricPathHop::new(FabricLinkId::new("runtime-sidecar").unwrap(), 3, 8).unwrap(),
        ])
        .unwrap()
    }

    fn packet(id: u64) -> FabricPacket {
        FabricPacket::new(FabricPacketId::new(id), 8, VirtualNetworkId::new(0)).unwrap()
    }

    fn capture(port: &FabricCheckpointPort) -> CheckpointRegistry {
        let mut registry = CheckpointRegistry::new();
        port.register(&mut registry).unwrap();
        port.capture_into(&mut registry).unwrap();
        registry
    }

    #[test]
    fn fabric_runtime_sidecar_cannot_override_wire_snapshot() {
        let component = CheckpointComponentId::new("fabric-sidecar").unwrap();
        let fabric = Arc::new(Mutex::new(FabricModel::new()));
        let port = FabricCheckpointPort::new(component.clone(), Arc::clone(&fabric))
            .with_runtime_state(fabric_runtime_checkpoint_state(&fabric));
        let path = route();

        fabric
            .lock()
            .unwrap()
            .transmit(0, packet(1), path.clone())
            .unwrap();
        let early = capture(&port);
        let early_logs = fabric.lock().unwrap().runtime_logs();

        fabric
            .lock()
            .unwrap()
            .transmit(1, packet(2), path.clone())
            .unwrap();
        let mut mixed = capture(&port);
        let late_wire_snapshot = fabric.lock().unwrap().snapshot();
        mixed
            .write_chunk(
                &component,
                FABRIC_RUNTIME_STATE_CHUNK,
                early
                    .chunk(&component, FABRIC_RUNTIME_STATE_CHUNK)
                    .unwrap()
                    .to_vec(),
            )
            .unwrap();

        fabric.lock().unwrap().transmit(2, packet(3), path).unwrap();
        port.restore_from(&mixed).unwrap();

        assert_eq!(fabric.lock().unwrap().snapshot(), late_wire_snapshot);
        assert_eq!(fabric.lock().unwrap().runtime_logs(), early_logs);
    }
}
