use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_interrupt::InterruptSourceId;
use rem6_memory::Address;
use rem6_mmio::MmioRoute;
use rem6_net::{SinicFifoDevice, SinicMmioDevice, SinicPciEndpointSpec, SinicRegisterParams};
use rem6_pci::{
    PciBarKind, PciConfigOffset, PciHostBarRange, PciHostBridge, PciLegacyInterruptRouter,
};

use crate::{
    PciHostCheckpointPort, PciLegacyInterruptRouterCheckpointPort, SinicFifoCheckpointPort,
    SinicRegisterCheckpointPort,
};

use super::{RiscvTopologySystem, RiscvTopologySystemError};

#[derive(Clone)]
pub struct RiscvTopologySinicPciDeviceConfig {
    spec: SinicPciEndpointSpec,
    pci_host: Arc<Mutex<PciHostBridge>>,
    legacy_interrupt_router: Arc<Mutex<PciLegacyInterruptRouter>>,
    bar_base: Address,
    mmio_route: MmioRoute,
    interrupt_source: InterruptSourceId,
    register_params: SinicRegisterParams,
    register_checkpoint_component: CheckpointComponentId,
    fifo_checkpoint_component: CheckpointComponentId,
    pci_host_checkpoint_component: CheckpointComponentId,
    pci_legacy_interrupt_router_checkpoint_component: CheckpointComponentId,
}

impl RiscvTopologySinicPciDeviceConfig {
    pub fn new(
        spec: SinicPciEndpointSpec,
        pci_host: Arc<Mutex<PciHostBridge>>,
        legacy_interrupt_router: Arc<Mutex<PciLegacyInterruptRouter>>,
        bar_base: Address,
        mmio_route: MmioRoute,
        interrupt_source: InterruptSourceId,
        register_params: SinicRegisterParams,
    ) -> Self {
        Self {
            spec,
            pci_host,
            legacy_interrupt_router,
            bar_base,
            mmio_route,
            interrupt_source,
            register_params,
            register_checkpoint_component: default_sinic_register_checkpoint_component(
                spec.function(),
            ),
            fifo_checkpoint_component: default_sinic_fifo_checkpoint_component(spec.function()),
            pci_host_checkpoint_component: default_pci_host_checkpoint_component(spec.function()),
            pci_legacy_interrupt_router_checkpoint_component:
                default_pci_legacy_interrupt_router_checkpoint_component(spec.function()),
        }
    }

    pub const fn spec(&self) -> SinicPciEndpointSpec {
        self.spec
    }

    pub fn pci_host(&self) -> Arc<Mutex<PciHostBridge>> {
        Arc::clone(&self.pci_host)
    }

    pub fn legacy_interrupt_router(&self) -> Arc<Mutex<PciLegacyInterruptRouter>> {
        Arc::clone(&self.legacy_interrupt_router)
    }

    pub const fn bar_base(&self) -> Address {
        self.bar_base
    }

    pub const fn mmio_route(&self) -> MmioRoute {
        self.mmio_route
    }

    pub const fn interrupt_source(&self) -> InterruptSourceId {
        self.interrupt_source
    }

    pub const fn register_params(&self) -> SinicRegisterParams {
        self.register_params
    }

    pub fn register_checkpoint_component(&self) -> &CheckpointComponentId {
        &self.register_checkpoint_component
    }

    pub fn fifo_checkpoint_component(&self) -> &CheckpointComponentId {
        &self.fifo_checkpoint_component
    }

    pub fn pci_host_checkpoint_component(&self) -> &CheckpointComponentId {
        &self.pci_host_checkpoint_component
    }

    pub fn pci_legacy_interrupt_router_checkpoint_component(&self) -> &CheckpointComponentId {
        &self.pci_legacy_interrupt_router_checkpoint_component
    }

    pub fn with_register_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.register_checkpoint_component = component;
        self
    }

    pub fn with_fifo_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.fifo_checkpoint_component = component;
        self
    }

    pub fn with_pci_host_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.pci_host_checkpoint_component = component;
        self
    }

    pub fn with_pci_legacy_interrupt_router_checkpoint_component(
        mut self,
        component: CheckpointComponentId,
    ) -> Self {
        self.pci_legacy_interrupt_router_checkpoint_component = component;
        self
    }
}

impl RiscvTopologySystem {
    pub fn with_sinic_pci_device(
        mut self,
        config: RiscvTopologySinicPciDeviceConfig,
    ) -> Result<Self, RiscvTopologySystemError> {
        let platform = self
            .platform
            .take()
            .ok_or(RiscvTopologySystemError::MissingPlatform)?;
        let device = Arc::new(Mutex::new(
            SinicFifoDevice::new(config.register_params())
                .map_err(RiscvTopologySystemError::Sinic)?,
        ));
        let mut endpoint = config
            .spec()
            .build_endpoint()
            .map_err(RiscvTopologySystemError::Sinic)?;

        {
            config
                .legacy_interrupt_router()
                .lock()
                .expect("SINIC PCI legacy interrupt router lock")
                .assign_endpoint_interrupt_line(&mut endpoint)
                .map_err(RiscvTopologySystemError::Pci)?;
        }
        {
            let host = config.pci_host();
            let mut host = host.lock().expect("SINIC PCI host bridge lock");
            host.register_endpoint(endpoint)
                .map_err(RiscvTopologySystemError::Pci)?;
            program_bar(&mut host, config.spec(), config.bar_base())?;
        }

        let host_bar_range = active_bar_range(&config)?;
        let interrupt_port = {
            let host = config.pci_host();
            let router = config.legacy_interrupt_router();
            let host = host.lock().expect("SINIC PCI host bridge lock");
            let router = router
                .lock()
                .expect("SINIC PCI legacy interrupt router lock");
            let port = router
                .port_for_host_endpoint(&host, config.spec().function())
                .map_err(RiscvTopologySystemError::Pci)?;
            config
                .spec()
                .build_legacy_interrupt_port(port, config.interrupt_source())
                .map_err(RiscvTopologySystemError::Sinic)?
        };
        let sinic_mmio = SinicMmioDevice::from_shared(Address::new(0), Arc::clone(&device))
            .with_pci_interrupt_port(interrupt_port);
        let bar_device = config
            .spec()
            .build_forwarded_bar_mmio_device(config.pci_host(), host_bar_range.clone(), sinic_mmio)
            .map_err(RiscvTopologySystemError::Sinic)?;
        self.platform = Some(
            platform
                .with_mmio_device(host_bar_range.host_range(), config.mmio_route(), bar_device)
                .map_err(RiscvTopologySystemError::Platform)?,
        );

        self = self.with_sinic_register_checkpoint_port(
            SinicRegisterCheckpointPort::from_fifo_device(
                config.register_checkpoint_component().clone(),
                Arc::clone(&device),
            ),
        )?;
        self = self.with_sinic_fifo_checkpoint_port(SinicFifoCheckpointPort::new(
            config.fifo_checkpoint_component().clone(),
            Arc::clone(&device),
        ))?;
        self = self.with_pci_host_checkpoint_port(PciHostCheckpointPort::new(
            config.pci_host_checkpoint_component().clone(),
            config.pci_host(),
        ))?;
        self = self.with_pci_legacy_interrupt_router_checkpoint_port(
            PciLegacyInterruptRouterCheckpointPort::new(
                config
                    .pci_legacy_interrupt_router_checkpoint_component()
                    .clone(),
                config.legacy_interrupt_router(),
            ),
        )?;
        Ok(self)
    }
}

fn program_bar(
    host: &mut PciHostBridge,
    spec: SinicPciEndpointSpec,
    bar_base: Address,
) -> Result<(), RiscvTopologySystemError> {
    let host_base = match spec.bar_kind() {
        PciBarKind::Memory32 {
            prefetchable: false,
        } => host.address_bases().memory_base(),
        PciBarKind::Memory32 { prefetchable: true } => {
            host.address_bases().prefetchable_memory_base()
        }
        PciBarKind::Memory64 {
            prefetchable: false,
        } => host.address_bases().memory_base(),
        PciBarKind::Memory64 { prefetchable: true } => {
            host.address_bases().prefetchable_memory_base()
        }
        PciBarKind::LegacyIo { .. } | PciBarKind::Io => host.address_bases().io_base(),
    };
    let pci_bar_base = bar_base.get().checked_sub(host_base.get()).ok_or(
        RiscvTopologySystemError::SinicPciBarAddressBelowHostBase {
            address: bar_base,
            host_base,
        },
    )?;
    let bar_value = u32::try_from(pci_bar_base)
        .map_err(|_| RiscvTopologySystemError::SinicPciBarAddressTooWide { address: bar_base })?;
    let bar_offset = PciConfigOffset::new(0x10).map_err(RiscvTopologySystemError::Pci)?;
    let command_offset = PciConfigOffset::new(0x04).map_err(RiscvTopologySystemError::Pci)?;
    let bar_address = host
        .aperture()
        .config_address(spec.function(), bar_offset)
        .map_err(RiscvTopologySystemError::Pci)?;
    host.write_config_address(bar_address, &bar_value.to_le_bytes())
        .map_err(RiscvTopologySystemError::Pci)?;
    let command_address = host
        .aperture()
        .config_address(spec.function(), command_offset)
        .map_err(RiscvTopologySystemError::Pci)?;
    host.write_config_address(command_address, &0x0002_u16.to_le_bytes())
        .map_err(RiscvTopologySystemError::Pci)
}

fn active_bar_range(
    config: &RiscvTopologySinicPciDeviceConfig,
) -> Result<PciHostBarRange, RiscvTopologySystemError> {
    let ranges = config
        .pci_host()
        .lock()
        .expect("SINIC PCI host bridge lock")
        .active_host_bar_ranges()
        .map_err(RiscvTopologySystemError::Pci)?;
    let bar = config.spec().bar_index();
    ranges
        .into_iter()
        .find(|range| range.function() == config.spec().function() && range.bar() == bar)
        .ok_or(RiscvTopologySystemError::MissingSinicPciBarRange {
            function: config.spec().function(),
            bar,
        })
}

fn default_sinic_register_checkpoint_component(
    function: rem6_pci::PciFunctionAddress,
) -> CheckpointComponentId {
    CheckpointComponentId::new(format!(
        "net.sinic.{}.{}.{}.registers",
        function.bus(),
        function.device(),
        function.function()
    ))
    .expect("formatted SINIC register checkpoint component is nonempty")
}

fn default_sinic_fifo_checkpoint_component(
    function: rem6_pci::PciFunctionAddress,
) -> CheckpointComponentId {
    CheckpointComponentId::new(format!(
        "net.sinic.{}.{}.{}.fifo",
        function.bus(),
        function.device(),
        function.function()
    ))
    .expect("formatted SINIC FIFO checkpoint component is nonempty")
}

fn default_pci_host_checkpoint_component(
    function: rem6_pci::PciFunctionAddress,
) -> CheckpointComponentId {
    CheckpointComponentId::new(format!(
        "pci.host.{}.{}.{}",
        function.bus(),
        function.device(),
        function.function()
    ))
    .expect("formatted PCI host checkpoint component is nonempty")
}

fn default_pci_legacy_interrupt_router_checkpoint_component(
    function: rem6_pci::PciFunctionAddress,
) -> CheckpointComponentId {
    CheckpointComponentId::new(format!(
        "pci.intx-router.{}.{}.{}",
        function.bus(),
        function.device(),
        function.function()
    ))
    .expect("formatted PCI legacy interrupt router checkpoint component is nonempty")
}
