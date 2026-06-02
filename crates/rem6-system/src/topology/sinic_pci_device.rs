use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::CheckpointComponentId;
use rem6_interrupt::InterruptSourceId;
use rem6_memory::Address;
use rem6_mmio::{MmioRoute, UnsupportedMmioDevice};
use rem6_net::{SinicFifoDevice, SinicMmioDevice, SinicPciEndpointSpec, SinicRegisterParams};
use rem6_pci::{
    PciBarKind, PciConfigOffset, PciHostAddressSpace, PciHostBarRange, PciHostBridge,
    PciLegacyInterruptRouter,
};
use rem6_platform::PlatformError;

use crate::{
    PciHostCheckpointPort, PciLegacyInterruptRouterCheckpointPort, SinicFifoCheckpointPort,
    SinicRegisterCheckpointPort, SystemError,
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
        self,
        config: RiscvTopologySinicPciDeviceConfig,
    ) -> Result<Self, RiscvTopologySystemError> {
        let platform = self
            .platform
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingPlatform)?;
        self.validate_sinic_pci_checkpoint_components(&config)?;
        let host_bar_range = {
            let host = config.pci_host();
            let host = host.lock().expect("SINIC PCI host bridge lock");
            host_bar_range_from_config(&host, config.spec(), config.bar_base())?
        };
        let precheck_device = UnsupportedMmioDevice::new(
            "sinic-pci-precheck",
            host_bar_range.host_range().start(),
            host_bar_range.host_range().size(),
        )
        .map_err(PlatformError::Mmio)
        .map_err(RiscvTopologySystemError::Platform)?;
        platform
            .clone()
            .with_mmio_device(
                host_bar_range.host_range(),
                config.mmio_route(),
                precheck_device,
            )
            .map_err(RiscvTopologySystemError::Platform)?;

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
        let pci_host = config.pci_host();
        let host_snapshot = pci_host
            .lock()
            .expect("SINIC PCI host bridge lock")
            .snapshot();
        let result = self.attach_sinic_pci_device_after_prevalidation(
            config,
            device,
            endpoint,
            host_bar_range,
        );
        match result {
            Ok(system) => Ok(system),
            Err(error) => {
                pci_host
                    .lock()
                    .expect("SINIC PCI host bridge lock")
                    .restore(&host_snapshot)
                    .expect("SINIC PCI host bridge restores pre-attach snapshot");
                Err(error)
            }
        }
    }

    fn validate_sinic_pci_checkpoint_components(
        &self,
        config: &RiscvTopologySinicPciDeviceConfig,
    ) -> Result<(), RiscvTopologySystemError> {
        let mut seen = BTreeSet::new();
        for component in [
            config.register_checkpoint_component(),
            config.fifo_checkpoint_component(),
            config.pci_host_checkpoint_component(),
            config.pci_legacy_interrupt_router_checkpoint_component(),
        ] {
            if !seen.insert(component.clone())
                || self.sinic_register_checkpoint_ports.contains_key(component)
                || self.sinic_fifo_checkpoint_ports.contains_key(component)
                || self.pci_host_checkpoint_ports.contains_key(component)
                || self
                    .pci_legacy_interrupt_router_checkpoint_ports
                    .contains_key(component)
            {
                return Err(
                    RiscvTopologySystemError::DuplicateSinicPciCheckpointComponent {
                        component: component.clone(),
                    },
                );
            }
            if self.host.as_ref().is_some_and(|host| {
                host.controller
                    .lock()
                    .expect("topology host controller lock")
                    .executor()
                    .has_checkpoint_component(component)
            }) {
                return Err(
                    RiscvTopologySystemError::DuplicateSinicPciCheckpointComponent {
                        component: component.clone(),
                    },
                );
            }
        }
        Ok(())
    }

    fn attach_sinic_pci_device_after_prevalidation(
        mut self,
        config: RiscvTopologySinicPciDeviceConfig,
        device: Arc<Mutex<SinicFifoDevice>>,
        endpoint: rem6_pci::PciEndpointConfig,
        host_bar_range: PciHostBarRange,
    ) -> Result<Self, RiscvTopologySystemError> {
        {
            let host = config.pci_host();
            let mut host = host.lock().expect("SINIC PCI host bridge lock");
            host.register_endpoint(endpoint)
                .map_err(RiscvTopologySystemError::Pci)?;
            program_bar(&mut host, config.spec(), &host_bar_range)?;
            validate_active_bar_range(&host, &host_bar_range)?;
        }

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
            self.platform
                .expect("SINIC PCI platform was prevalidated")
                .with_mmio_device(host_bar_range.host_range(), config.mmio_route(), bar_device)
                .map_err(RiscvTopologySystemError::Platform)?,
        );

        self.attach_sinic_pci_checkpoint_ports(&config, device)?;
        Ok(self)
    }

    fn attach_sinic_pci_checkpoint_ports(
        &mut self,
        config: &RiscvTopologySinicPciDeviceConfig,
        device: Arc<Mutex<SinicFifoDevice>>,
    ) -> Result<(), RiscvTopologySystemError> {
        let register_port = SinicRegisterCheckpointPort::from_fifo_device(
            config.register_checkpoint_component().clone(),
            Arc::clone(&device),
        );
        let fifo_port = SinicFifoCheckpointPort::new(
            config.fifo_checkpoint_component().clone(),
            Arc::clone(&device),
        )
        .with_register_checkpoint_component(config.register_checkpoint_component().clone());
        let pci_host_port = PciHostCheckpointPort::new(
            config.pci_host_checkpoint_component().clone(),
            config.pci_host(),
        );
        let router_port = PciLegacyInterruptRouterCheckpointPort::new(
            config
                .pci_legacy_interrupt_router_checkpoint_component()
                .clone(),
            config.legacy_interrupt_router(),
        );

        if let Some(host) = self.host.as_ref() {
            let mut controller = host
                .controller
                .lock()
                .expect("topology host controller lock");
            let executor = controller.executor_mut();
            executor
                .attach_sinic_register_checkpoint_port(register_port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
            executor
                .attach_sinic_fifo_checkpoint_port(fifo_port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
            executor
                .attach_pci_host_checkpoint_port(pci_host_port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
            executor
                .attach_pci_legacy_interrupt_router_checkpoint_port(router_port.clone())
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        self.sinic_register_checkpoint_ports.insert(
            config.register_checkpoint_component().clone(),
            register_port,
        );
        self.sinic_fifo_checkpoint_ports
            .insert(config.fifo_checkpoint_component().clone(), fifo_port);
        self.pci_host_checkpoint_ports.insert(
            config.pci_host_checkpoint_component().clone(),
            pci_host_port,
        );
        self.pci_legacy_interrupt_router_checkpoint_ports.insert(
            config
                .pci_legacy_interrupt_router_checkpoint_component()
                .clone(),
            router_port,
        );
        Ok(())
    }
}

fn program_bar(
    host: &mut PciHostBridge,
    spec: SinicPciEndpointSpec,
    host_bar_range: &PciHostBarRange,
) -> Result<(), RiscvTopologySystemError> {
    let bar_value = u32::try_from(host_bar_range.pci_range().start().get()).map_err(|_| {
        RiscvTopologySystemError::SinicPciBarAddressTooWide {
            address: host_bar_range.host_range().start(),
        }
    })?;
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

fn validate_active_bar_range(
    host: &PciHostBridge,
    expected: &PciHostBarRange,
) -> Result<(), RiscvTopologySystemError> {
    if host
        .active_host_bar_ranges()
        .map_err(RiscvTopologySystemError::Pci)?
        .iter()
        .any(|range| range == expected)
    {
        return Ok(());
    }
    Err(RiscvTopologySystemError::MissingSinicPciBarRange {
        function: expected.function(),
        bar: expected.bar(),
    })
}

fn host_bar_range_from_config(
    host: &PciHostBridge,
    spec: SinicPciEndpointSpec,
    bar_base: Address,
) -> Result<PciHostBarRange, RiscvTopologySystemError> {
    let (space, host_base) = host_space_and_base(host, spec.bar_kind());
    let pci_bar_base = bar_base.get().checked_sub(host_base.get()).ok_or(
        RiscvTopologySystemError::SinicPciBarAddressBelowHostBase {
            address: bar_base,
            host_base,
        },
    )?;
    let alignment_bytes = spec.bar_size().bytes();
    if !pci_bar_base.is_multiple_of(alignment_bytes) {
        return Err(RiscvTopologySystemError::SinicPciBarAddressMisaligned {
            address: bar_base,
            alignment_bytes,
        });
    }
    let is_64_bit_bar = matches!(spec.bar_kind(), PciBarKind::Memory64 { .. });
    if !is_64_bit_bar && pci_bar_base > u32::MAX as u64 {
        return Err(RiscvTopologySystemError::SinicPciBarAddressTooWide { address: bar_base });
    }
    PciHostBarRange::new(
        spec.function(),
        spec.bar_index(),
        space,
        Address::new(pci_bar_base),
        bar_base,
        spec.bar_size(),
    )
    .map_err(RiscvTopologySystemError::Pci)
}

fn host_space_and_base(host: &PciHostBridge, kind: PciBarKind) -> (PciHostAddressSpace, Address) {
    match kind {
        PciBarKind::Memory32 {
            prefetchable: false,
        }
        | PciBarKind::Memory64 {
            prefetchable: false,
        } => (
            PciHostAddressSpace::Memory,
            host.address_bases().memory_base(),
        ),
        PciBarKind::Memory32 { prefetchable: true }
        | PciBarKind::Memory64 { prefetchable: true } => (
            PciHostAddressSpace::PrefetchableMemory,
            host.address_bases().prefetchable_memory_base(),
        ),
        PciBarKind::LegacyIo { .. } | PciBarKind::Io => {
            (PciHostAddressSpace::Io, host.address_bases().io_base())
        }
    }
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
