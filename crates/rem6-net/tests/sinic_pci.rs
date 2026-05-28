use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address};
use rem6_mmio::{MmioBus, MmioCompletion, MmioRequest, MmioRequestId, MmioResponse, MmioRoute};
use rem6_net::{
    SinicFifoDevice, SinicInterrupts, SinicMmioDevice, SinicPciEndpointSpec, SinicRegisterParams,
};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciClassCode, PciConfigAperture, PciConfigOffset, PciDeviceIdentity,
    PciFunctionAddress, PciHostAddressBases, PciHostBarRange, PciHostBridge,
};

fn function() -> PciFunctionAddress {
    PciFunctionAddress::new(0, 9, 0).unwrap()
}

fn read_request(id: u64, address: u64, bytes: u64) -> MmioRequest {
    MmioRequest::read(
        MmioRequestId::new(id),
        Address::new(address),
        AccessSize::new(bytes).unwrap(),
    )
    .unwrap()
}

fn write_request(id: u64, address: u64, data: Vec<u8>) -> MmioRequest {
    MmioRequest::write(
        MmioRequestId::new(id),
        Address::new(address),
        data.clone(),
        rem6_memory::ByteMask::full(AccessSize::new(data.len() as u64).unwrap()).unwrap(),
    )
    .unwrap()
}

fn pci_host() -> PciHostBridge {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let mut host = PciHostBridge::with_address_bases(
        aperture,
        PciHostAddressBases::new(
            Address::new(0x1000_0000),
            Address::new(0x8000_0000),
            Address::new(0x9000_0000),
        ),
    );
    let endpoint = SinicPciEndpointSpec::new(function())
        .build_endpoint()
        .unwrap();
    host.register_endpoint(endpoint).unwrap();
    host
}

fn active_host_bar_range(host: &mut PciHostBridge) -> PciHostBarRange {
    let function = function();
    let bar = PciBarIndex::new(0).unwrap();
    let config = host
        .aperture()
        .config_address(function, PciConfigOffset::new(0x10).unwrap())
        .unwrap();
    host.write_config_address(config, &0x8000_0000_u32.to_le_bytes())
        .unwrap();
    host.write_config_address(
        host.aperture()
            .config_address(function, PciConfigOffset::new(0x04).unwrap())
            .unwrap(),
        &0x0002_u16.to_le_bytes(),
    )
    .unwrap();
    let ranges = host.active_host_bar_ranges().unwrap();
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0].function(), function);
    assert_eq!(ranges[0].bar(), bar);
    ranges[0].clone()
}

#[test]
fn sinic_pci_endpoint_matches_gem5_identity_and_bar_shape() {
    let spec = SinicPciEndpointSpec::new(function());
    assert_eq!(spec.function(), function());
    assert_eq!(spec.bar_index(), PciBarIndex::new(0).unwrap());
    assert_eq!(
        spec.bar_kind(),
        PciBarKind::Memory32 {
            prefetchable: false
        }
    );
    assert_eq!(spec.bar_size(), AccessSize::new(0x1_0000).unwrap());
    assert_eq!(spec.identity(), PciDeviceIdentity::new(0x1291, 0x1293));
    assert_eq!(spec.class(), PciClassCode::new(0x02, 0x00, 0x00, 0x00));

    let mut endpoint = spec.build_endpoint().unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x0c).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, 0x00, 0x00, 0x00])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x3e).unwrap(),
            AccessSize::new(2).unwrap()
        ),
        Ok(vec![0xb0, 0x34])
    );
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x10).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, 0x00, 0x00, 0x00])
    );
    endpoint
        .write_config(
            PciConfigOffset::new(0x10).unwrap(),
            &0xffff_ffff_u32.to_le_bytes(),
        )
        .unwrap();
    assert_eq!(
        endpoint.read_config(
            PciConfigOffset::new(0x10).unwrap(),
            AccessSize::new(4).unwrap()
        ),
        Ok(vec![0x00, 0x00, 0xff, 0xff])
    );
}

#[test]
fn sinic_pci_bar_routes_host_mmio_to_sinic_registers() {
    let mut host = pci_host();
    let range = active_host_bar_range(&mut host);
    let endpoint = SinicMmioDevice::new(
        Address::new(0),
        SinicFifoDevice::new(
            SinicRegisterParams::default()
                .with_interrupt_mask(SinicInterrupts::SOFT | SinicInterrupts::RX_DMA),
        )
        .unwrap(),
    );
    let bar_device = SinicPciEndpointSpec::new(function())
        .build_bar_mmio_device(range.clone(), endpoint)
        .unwrap();

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(range.host_range(), route, bar_device)
        .unwrap();
    let bus = Arc::new(bus);
    let host_start = range.host_range().start().get();
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let read_bus = Arc::clone(&bus);
    let read_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 4, move |context| {
            read_bus
                .submit_parallel(
                    context,
                    read_request(1, host_start + 0x08, 4),
                    move |completion| read_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let write_bus = Arc::clone(&bus);
    let write_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            write_bus
                .submit_parallel(
                    context,
                    write_request(2, host_start + 0x04, 0x0000_0002_u32.to_le_bytes().to_vec()),
                    move |completion| write_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();
    let completions = completions.lock().unwrap();
    assert_eq!(
        response_for(&completions, MmioRequestId::new(1)),
        &Ok(MmioResponse::completed(
            MmioRequestId::new(1),
            Some(vec![0x00, 0x00, 0x00, 0x00]),
        ))
    );
    assert_eq!(
        response_for(&completions, MmioRequestId::new(2)),
        &Ok(MmioResponse::completed(MmioRequestId::new(2), None))
    );
}

fn response_for(
    completions: &[MmioCompletion],
    request: MmioRequestId,
) -> &Result<MmioResponse, rem6_mmio::MmioError> {
    completions
        .iter()
        .find_map(|completion| match completion.response() {
            Ok(response) if response.request() == request => Some(completion.response()),
            _ => None,
        })
        .expect("completion for request")
}
