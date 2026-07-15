use rem6_system::RiscvDataCacheProtocol;

#[test]
fn riscv_data_cache_protocol_names_round_trip() {
    let expected = [
        (RiscvDataCacheProtocol::Msi, "msi"),
        (RiscvDataCacheProtocol::Mesi, "mesi"),
        (RiscvDataCacheProtocol::Moesi, "moesi"),
        (RiscvDataCacheProtocol::Chi, "chi"),
    ];

    assert_eq!(
        RiscvDataCacheProtocol::ALL,
        expected.map(|(protocol, _)| protocol)
    );
    for (protocol, name) in expected {
        assert_eq!(protocol.as_str(), name);
        assert_eq!(RiscvDataCacheProtocol::parse(name), Some(protocol));
    }
}

#[test]
fn riscv_data_cache_protocol_parse_rejects_noncanonical_names() {
    for value in ["", "MSI", "mesi ", "directory"] {
        assert_eq!(RiscvDataCacheProtocol::parse(value), None);
    }
}
