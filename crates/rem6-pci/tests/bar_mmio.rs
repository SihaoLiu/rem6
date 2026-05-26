use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AddressRange, ByteMask};
use rem6_mmio::{
    MmioAccess, MmioBus, MmioCompletion, MmioError, MmioRegisterBank, MmioRequest, MmioRequestId,
    MmioResponse, MmioRoute,
};
use rem6_pci::{
    PciBarIndex, PciBarKind, PciBarMmioDevice, PciBarSpec, PciClassCode, PciConfigAperture,
    PciConfigOffset, PciDeviceIdentity, PciEndpointConfig, PciFunctionAddress, PciHostAddressBases,
    PciHostBarRange, PciHostBridge,
};

fn endpoint(function: PciFunctionAddress) -> PciEndpointConfig {
    let mut endpoint = PciEndpointConfig::new(
        function,
        PciDeviceIdentity::new(0x1af4, 0x1001),
        PciClassCode::new(0x01, 0x00, 0x00, 0x00),
    );
    endpoint
        .install_bar(
            PciBarSpec::new(
                PciBarIndex::new(0).unwrap(),
                PciBarKind::Memory32 {
                    prefetchable: false,
                },
                AccessSize::new(0x100).unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
    endpoint
}

fn active_bar_range() -> PciHostBarRange {
    let aperture = PciConfigAperture::ecam(Address::new(0x3000_0000), 1).unwrap();
    let function = PciFunctionAddress::new(0, 5, 0).unwrap();
    let mut host = PciHostBridge::with_address_bases(
        aperture,
        PciHostAddressBases::new(
            Address::new(0x1000_0000),
            Address::new(0x8000_0000),
            Address::new(0xa000_0000),
        ),
    );
    host.register_endpoint(endpoint(function)).unwrap();
    host.write_config_address(
        aperture
            .config_address(function, PciConfigOffset::new(0x10).unwrap())
            .unwrap(),
        &0x0000_2000_u32.to_le_bytes(),
    )
    .unwrap();
    host.write_config_address(
        aperture
            .config_address(function, PciConfigOffset::new(0x04).unwrap())
            .unwrap(),
        &0x0002_u16.to_le_bytes(),
    )
    .unwrap();

    let ranges = host.active_host_bar_ranges().unwrap();
    assert_eq!(ranges.len(), 1);
    ranges[0].clone()
}

fn local_register_bank() -> Mutex<MmioRegisterBank> {
    let mut bank = MmioRegisterBank::new(Address::new(0), AccessSize::new(0x100).unwrap()).unwrap();
    bank.insert_register(
        0x20,
        AccessSize::new(4).unwrap(),
        MmioAccess::ReadWrite,
        vec![0x10, 0x20, 0x30, 0x40],
    )
    .unwrap();
    Mutex::new(bank)
}

fn response_for(
    completions: &[MmioCompletion],
    request: MmioRequestId,
) -> &Result<MmioResponse, MmioError> {
    completions
        .iter()
        .find_map(|completion| match completion.response() {
            Ok(response) if response.request() == request => Some(completion.response()),
            Err(MmioError::DeviceBoundaryCrossed {
                request: failed, ..
            }) if *failed == request => Some(completion.response()),
            _ => None,
        })
        .expect("completion for request")
}

fn completed_data(completions: &[MmioCompletion], request: MmioRequestId) -> Vec<u8> {
    response_for(completions, request)
        .as_ref()
        .expect("successful MMIO response")
        .data()
        .expect("read response data")
        .to_vec()
}

#[test]
fn pci_bar_mmio_device_routes_serial_host_access_to_bar_local_offset() {
    let range = active_bar_range();
    let device = PciBarMmioDevice::new(range.clone(), local_register_bank());
    assert_eq!(
        device.host_range(),
        AddressRange::new(Address::new(0x8000_2000), AccessSize::new(0x100).unwrap()).unwrap()
    );
    assert_eq!(
        device.local_range(),
        AddressRange::new(Address::new(0), AccessSize::new(0x100).unwrap()).unwrap()
    );

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(device.host_range(), route, device)
        .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::new(2).unwrap();
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_at(cpu, 5, move |context| {
            bus.submit(
                context,
                MmioRequest::read(
                    MmioRequestId::new(1),
                    Address::new(range.host_range().start().get() + 0x20),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle();

    let completions = completions.lock().unwrap();
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(1)),
        vec![0x10, 0x20, 0x30, 0x40]
    );
}

#[test]
fn pci_bar_mmio_device_routes_parallel_host_writes_to_bar_local_offset() {
    let range = active_bar_range();
    let host_start = range.host_range().start();
    let device = PciBarMmioDevice::new(range.clone(), local_register_bank());

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 2, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(device.host_range(), route, device)
        .unwrap();
    let bus = Arc::new(bus);
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let write_bus = Arc::clone(&bus);
    let write_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            write_bus
                .submit_parallel(
                    context,
                    MmioRequest::write(
                        MmioRequestId::new(2),
                        Address::new(host_start.get() + 0x21),
                        vec![0xaa, 0xbb],
                        ByteMask::from_bits(vec![true, false]).unwrap(),
                    )
                    .unwrap(),
                    move |completion| write_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    let read_bus = Arc::clone(&bus);
    let read_completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 10, move |context| {
            read_bus
                .submit_parallel(
                    context,
                    MmioRequest::read(
                        MmioRequestId::new(3),
                        Address::new(host_start.get() + 0x20),
                        AccessSize::new(4).unwrap(),
                    )
                    .unwrap(),
                    move |completion| read_completed.lock().unwrap().push(completion),
                )
                .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap();
    assert_eq!(
        response_for(&completions, MmioRequestId::new(2))
            .as_ref()
            .unwrap()
            .data(),
        None
    );
    assert_eq!(
        completed_data(&completions, MmioRequestId::new(3)),
        vec![0x10, 0xaa, 0x30, 0x40]
    );
}

#[test]
fn pci_bar_mmio_device_rejects_host_accesses_outside_the_bar_range() {
    let range = active_bar_range();
    let device = PciBarMmioDevice::new(range.clone(), local_register_bank());

    let cpu = PartitionId::new(0);
    let pci = PartitionId::new(1);
    let route = MmioRoute::new(cpu, pci, 1, 1).unwrap();
    let mut bus = MmioBus::new();
    bus.insert_device(
        AddressRange::new(range.host_range().start(), AccessSize::new(0x120).unwrap()).unwrap(),
        route,
        device,
    )
    .unwrap();
    let completions = Arc::new(Mutex::new(Vec::new()));

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 1).unwrap();
    let completed = Arc::clone(&completions);
    scheduler
        .schedule_parallel_at(cpu, 5, move |context| {
            bus.submit_parallel(
                context,
                MmioRequest::read(
                    MmioRequestId::new(4),
                    Address::new(range.host_range().end().get() - 2),
                    AccessSize::new(4).unwrap(),
                )
                .unwrap(),
                move |completion| completed.lock().unwrap().push(completion),
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_parallel().unwrap();

    let completions = completions.lock().unwrap();
    assert_eq!(
        response_for(&completions, MmioRequestId::new(4)),
        &Err(MmioError::DeviceBoundaryCrossed {
            request: MmioRequestId::new(4),
            device_start: Address::new(0x8000_2000),
            device_end: Address::new(0x8000_2100),
            requested_start: Address::new(0x8000_20fe),
            requested_end: Address::new(0x8000_2102),
        })
    );
}
