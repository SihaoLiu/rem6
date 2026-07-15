use rem6_workload::WorkloadDataCacheProtocol;

#[test]
fn workload_data_cache_protocol_names_round_trip() {
    let expected = [
        (WorkloadDataCacheProtocol::Msi, "msi"),
        (WorkloadDataCacheProtocol::Mesi, "mesi"),
        (WorkloadDataCacheProtocol::Moesi, "moesi"),
        (WorkloadDataCacheProtocol::Chi, "chi"),
    ];

    assert_eq!(
        WorkloadDataCacheProtocol::ALL,
        expected.map(|(protocol, _)| protocol)
    );
    for (protocol, name) in expected {
        assert_eq!(protocol.as_str(), name);
        assert_eq!(WorkloadDataCacheProtocol::parse(name), Some(protocol));
    }
}

#[test]
fn workload_data_cache_protocol_parse_rejects_noncanonical_names() {
    for value in ["", "CHI", "moesi\n", "snoop"] {
        assert_eq!(WorkloadDataCacheProtocol::parse(value), None);
    }
}
