use std::collections::BTreeMap;

use rem6_kernel::ParallelSchedulerContext;
use rem6_memory::{AccessSize, AddressRange, MemoryOperation, MemoryRequest, MemoryResponse};
use rem6_mmio::{MmioRequest, MmioRequestId};
use rem6_net::{SinicFifoDevice, SinicMmioDevice, SinicRegisterParams};
use rem6_transport::{RequestDelivery, TargetOutcome, TransportEndpointId};
use rem6_workload::{WorkloadSinicPciDevice, WorkloadTopology};

use super::RiscvWorkloadReplayError;

#[derive(Clone, Debug)]
pub(super) struct WorkloadSinicPciMmioBackend {
    devices: BTreeMap<TransportEndpointId, WorkloadSinicPciMmioDevice>,
}

impl WorkloadSinicPciMmioBackend {
    pub(super) fn from_topology(
        topology: &WorkloadTopology,
    ) -> Result<Self, RiscvWorkloadReplayError> {
        let mut devices = BTreeMap::new();
        for device in topology.sinic_pci_devices() {
            let endpoint = TransportEndpointId::new(device.mmio_endpoint())
                .map_err(RiscvWorkloadReplayError::Transport)?;
            devices.insert(endpoint, WorkloadSinicPciMmioDevice::new(device));
        }
        Ok(Self { devices })
    }

    pub(super) fn respond_parallel(
        &self,
        delivery: &RequestDelivery,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Option<TargetOutcome> {
        let device = self.devices.get(delivery.endpoint())?;
        device.respond_parallel(delivery.request(), context)
    }

    pub(super) fn accepts_delivery(&self, delivery: &RequestDelivery) -> bool {
        self.devices
            .get(delivery.endpoint())
            .is_some_and(|device| device.accepts_request(delivery.request()))
    }
}

#[derive(Clone, Debug)]
struct WorkloadSinicPciMmioDevice {
    bar_range: AddressRange,
    device: SinicMmioDevice,
}

impl WorkloadSinicPciMmioDevice {
    fn new(device: &WorkloadSinicPciDevice) -> Self {
        let bar_size = AccessSize::new(WorkloadSinicPciDevice::BAR_BYTES)
            .expect("valid workload SINIC PCI BAR size");
        let bar_range =
            AddressRange::new(device.bar_base(), bar_size).expect("valid workload SINIC PCI BAR");
        let fifo = SinicFifoDevice::new(SinicRegisterParams::default())
            .expect("valid default SINIC register parameters");
        Self {
            bar_range,
            device: SinicMmioDevice::new(device.bar_base(), fifo),
        }
    }

    fn respond_parallel(
        &self,
        request: &MemoryRequest,
        context: &mut ParallelSchedulerContext<'_>,
    ) -> Option<TargetOutcome> {
        if !self.accepts_request(request) {
            return None;
        }
        let mmio_request = memory_request_to_mmio_request(request)
            .expect("workload SINIC PCI MMIO request maps to MMIO");
        let response = self
            .device
            .respond_parallel(context, &mmio_request)
            .expect("workload SINIC PCI MMIO response");
        Some(TargetOutcome::Respond(
            memory_response_from_mmio_response(request, response.data())
                .expect("workload SINIC PCI MMIO response maps to memory"),
        ))
    }

    fn accepts_request(&self, request: &MemoryRequest) -> bool {
        self.bar_range.contains_range(request.range())
    }
}

fn memory_request_to_mmio_request(request: &MemoryRequest) -> Result<MmioRequest, &'static str> {
    let request_id = MmioRequestId::new(request.id().sequence());
    match request.operation() {
        MemoryOperation::ReadShared | MemoryOperation::ReadUnique => {
            MmioRequest::read(request_id, request.range().start(), request.size())
                .map_err(|_| "invalid MMIO read request")
        }
        MemoryOperation::Write => {
            let data = request.data().ok_or("missing MMIO write data")?.to_vec();
            let byte_mask = request
                .byte_mask()
                .ok_or("missing MMIO write byte mask")?
                .clone();
            MmioRequest::write(request_id, request.range().start(), data, byte_mask)
                .map_err(|_| "invalid MMIO write request")
        }
        _ => Err("unsupported MMIO memory operation"),
    }
}

fn memory_response_from_mmio_response(
    request: &MemoryRequest,
    data: Option<&[u8]>,
) -> Result<MemoryResponse, rem6_memory::MemoryError> {
    MemoryResponse::completed(request, data.map(<[u8]>::to_vec))
}
