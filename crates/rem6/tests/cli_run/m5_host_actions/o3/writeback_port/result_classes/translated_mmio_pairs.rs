use super::super::result_support::{
    assert_event_order, assert_register, assert_register_absent, data_trace, event_str, json_u64,
    memory_dump_hex, memory_result_event_at_pc,
};
use super::super::*;
#[path = "translated_mmio_pairs/boundaries.rs"]
mod boundaries;
#[path = "translated_mmio_pairs/fixture.rs"]
mod fixture;
use fixture::{assert_route_resources, TranslatedMemoryPairFixture};
const FIRST_PC: &str = "0x80000030";
const SECOND_PC: &str = "0x80000034";
const DIV_PC: &str = "0x80000038";
const DEPENDENT_PC: &str = "0x8000003c";
const FIRST_VIRTUAL_PAGE: u64 = 0x4000;
const SECOND_VIRTUAL_PAGE: u64 = 0x5000;
const FIRST_PHYSICAL_PAGE: u64 = 0x8000_1000;
const SECOND_PHYSICAL_PAGE: u64 = 0x8000_2000;
const PAIR_MAX_TICK: u64 = 5_000;
const HIERARCHY_ROUTE_DELAY: u64 = 4;
const ROUTE_DELAY_CANDIDATES: [u64; 13] = [1, 2, 3, 4, 6, 8, 9, 10, 12, 14, 16, 20, 24];
const PAIR_PCS: [&str; 4] = [FIRST_PC, SECOND_PC, DIV_PC, DEPENDENT_PC];
static DIRECT_ROUTE_DELAY: std::sync::OnceLock<u64> = std::sync::OnceLock::new();
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RequestIdentity {
    agent: u64,
    sequence: u64,
}
struct PairRequestEvidence<'a> {
    pc: &'static str,
    physical_address: u64,
    event: &'a Value,
    sent: &'a Value,
    response: &'a Value,
    identity: RequestIdentity,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PcRequestIdentity {
    data: RequestIdentity,
    fetch: RequestIdentity,
}
#[test]
fn rem6_run_o3_translated_memory_result_pair_width_one_direct() {
    run_translated_memory_result_pair("direct", 1);
}
#[test]
fn rem6_run_o3_translated_memory_result_pair_width_two_exact_fit_direct() {
    run_translated_memory_result_pair("direct", 2);
}
#[test]
fn rem6_run_o3_translated_memory_result_pair_width_one_cache_fabric_dram() {
    run_translated_memory_result_pair("cache-fabric-dram", 1);
}
#[test]
fn rem6_run_o3_translated_memory_mmio_result_pair_width_one_direct() {
    run_translated_memory_mmio_result_pair("direct");
}
#[test]
fn rem6_run_o3_translated_memory_mmio_result_pair_width_one_cache_fabric_dram() {
    run_translated_memory_mmio_result_pair("cache-fabric-dram");
}
fn run_translated_memory_mmio_result_pair(memory_system: &str) {
    let calibration = TranslatedMemoryPairFixture::new();
    let route_delay = calibrated_route_delay(&calibration, memory_system);
    boundaries::assert_mixed_pair(
        &TranslatedMemoryPairFixture::new_mmio(),
        memory_system,
        route_delay,
    );
}
fn run_translated_memory_result_pair(memory_system: &str, writeback_width: usize) {
    let fixture = TranslatedMemoryPairFixture::new();
    let route_delay = calibrated_route_delay(&fixture, memory_system);
    let control = fixture.run_identity_control(memory_system, writeback_width, route_delay);
    let identities = pair_request_identities(&control);
    let completed = fixture.run(memory_system, writeback_width, route_delay, PAIR_MAX_TICK);
    let pair = pair_request_evidence(&completed, &identities);
    assert_first_translation_path(&pair[0]);
    assert_pair_request_identities(&completed, &pair, &identities);
    let earliest_response = pair
        .iter()
        .map(|evidence| event_u64(evidence.event, "lsq_data_response_tick"))
        .min()
        .unwrap();
    let resident = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        earliest_response.saturating_sub(1),
    );
    assert_two_outstanding_pair_requests(&resident, &pair, earliest_response);
    assert_pre_response_residency(&resident, event_u64(pair[0].event, "issue_tick"));
    assert_result_timing(&completed, memory_system, writeback_width);
    assert_oldest_first_commit(&completed);
    assert_final_witness(&completed);
    let before_pair = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        event_u64(pair[0].event, "issue_tick").saturating_sub(1),
    );
    let through_pair = fixture.run(
        memory_system,
        writeback_width,
        route_delay,
        pair.iter()
            .map(|evidence| event_u64(evidence.event, "lsq_data_response_tick"))
            .max()
            .unwrap()
            + 1,
    );
    assert_route_resources(&before_pair, &through_pair, memory_system, &pair);
}
fn calibrated_route_delay(fixture: &TranslatedMemoryPairFixture, memory_system: &str) -> u64 {
    match memory_system {
        "cache-fabric-dram" => HIERARCHY_ROUTE_DELAY,
        "direct" => *DIRECT_ROUTE_DELAY.get_or_init(|| {
            let mut matches = Vec::new();
            let mut observations = Vec::new();
            for route_delay in ROUTE_DELAY_CANDIDATES {
                let json = fixture.run_calibration("direct", route_delay);
                let first = memory_result_event_at_pc(&json, FIRST_PC);
                let second = memory_result_event_at_pc(&json, SECOND_PC);
                let div = event_at_pc(&json, DIV_PC);
                let first_raw_ready = event_u64(first, "lsq_data_response_tick") + 1;
                let second_raw_ready = event_u64(second, "lsq_data_response_tick") + 1;
                let div_raw_ready = event_u64(div, "issue_tick") + 19;
                observations.push((
                    route_delay,
                    first_raw_ready,
                    second_raw_ready,
                    div_raw_ready,
                ));
                if first_raw_ready == div_raw_ready && second_raw_ready == div_raw_ready {
                    matches.push(route_delay);
                }
            }
            assert_eq!(
                matches.len(),
                1,
                "direct pair must calibrate uniquely against both loads and DIV: {observations:?}"
            );
            matches[0]
        }),
        _ => panic!("unsupported translated pair memory system {memory_system}"),
    }
}
fn assert_first_translation_path(first: &PairRequestEvidence<'_>) {
    assert_eq!(first.pc, FIRST_PC);
    assert_eq!(first.physical_address, FIRST_PHYSICAL_PAGE);
    assert_eq!(event_str(first.sent, "endpoint"), "cpu0.dmem");
    assert_eq!(event_str(first.response, "endpoint"), "cpu0.dmem");
    assert_eq!(event_str(first.response, "response_status"), "completed");
    assert_eq!(
        event_u64(first.sent, "tick"),
        event_u64(first.event, "issue_tick")
    );
    assert_eq!(
        event_u64(first.response, "tick"),
        event_u64(first.event, "lsq_data_response_tick")
    );
}
fn assert_pair_request_identities(
    json: &Value,
    pair: &[PairRequestEvidence<'_>; 2],
    identities: &[PcRequestIdentity; 2],
) {
    for index in 0..2 {
        assert_eq!(pair[index].identity, identities[index].data);
        assert_eq!(
            fetch_request_identity(json, fetch_record_at_pc(json, PAIR_PCS[index])),
            identities[index].fetch
        );
    }
    assert_ne!(identities[0].data, identities[1].data);
    assert_ne!(identities[0].fetch, identities[1].fetch);
}
fn assert_pre_response_residency(json: &Value, first_issue: u64) {
    assert_eq!(json_u64(json, "/cores/0/o3_runtime/snapshot/rob/count"), 4);
    assert_eq!(json_u64(json, "/cores/0/o3_runtime/snapshot/lsq/count"), 2);
    let rows = PAIR_PCS
        .map(|pc| rob_entry_at_pc(json, pc))
        .map(|row| event_u64(row, "sequence"));
    assert!(rows.windows(2).all(|pair| pair[0] < pair[1]));
    let lsq = json
        .pointer("/cores/0/o3_runtime/snapshot/lsq/entries")
        .and_then(Value::as_array)
        .expect("translated pair LSQ snapshot");
    assert_eq!(
        lsq.iter()
            .map(|row| event_u64(row, "sequence"))
            .collect::<Vec<_>>(),
        rows[..2]
    );
    assert_register_absent(json, "x3");
    assert_register_absent(json, "x11");
    assert_register_absent(json, "x12");
    assert_register_absent(json, "x13");
    assert!(
        data_trace(json)
            .iter()
            .all(|record| event_u64(record, "tick") < first_issue),
        "only timing-mode setup traffic may complete before the pair response"
    );
    for pc in [FIRST_PC, SECOND_PC] {
        fetch_record_at_pc(json, pc);
    }
}
fn assert_two_outstanding_pair_requests(
    json: &Value,
    pair: &[PairRequestEvidence<'_>; 2],
    earliest_response: u64,
) {
    let first = request_sent_for_identity(json, pair[0].identity).unwrap_or_else(|| {
        panic!(
            "missing exact {FIRST_PC} translated request {:?}: {}",
            pair[0].identity,
            pair_request_trace(json)
        )
    });
    assert!(
        event_u64(first, "tick") < earliest_response,
        "exact {FIRST_PC} translated request {:?} issued too late: {first}",
        pair[0].identity
    );
    let second = request_sent_for_identity(json, pair[1].identity);
    assert!(
        second.is_some_and(|request| event_u64(request, "tick") < earliest_response),
        "translated result-pair gate blocked exact {SECOND_PC} request {:?} before response {earliest_response}; {FIRST_PC} {:?} issued at {}, completed {SECOND_PC} issue {}, observed pair requests {}",
        pair[1].identity,
        pair[0].identity,
        event_u64(first, "tick"),
        event_u64(pair[1].event, "issue_tick"),
        pair_request_trace(json),
    );
    assert_ne!(pair[0].identity, pair[1].identity);
}
fn assert_result_timing(json: &Value, memory_system: &str, writeback_width: usize) {
    let first = memory_result_event_at_pc(json, FIRST_PC);
    let second = memory_result_event_at_pc(json, SECOND_PC);
    let div = event_at_pc(json, DIV_PC);
    if memory_system == "cache-fabric-dram" {
        assert_eq!(writeback_width, 1);
        assert!(
            event_u64(first, "lsq_data_response_tick")
                < event_u64(second, "lsq_data_response_tick")
        );
        assert!(event_u64(first, "writeback_tick") <= event_u64(second, "writeback_tick"));
        assert!(event_u64(div, "writeback_tick") >= event_u64(div, "issue_tick") + 19);
        return;
    }
    let raw_ready = event_u64(div, "issue_tick") + 19;
    for result in [first, second] {
        assert_eq!(event_u64(result, "lsq_data_response_tick") + 1, raw_ready);
    }
    assert_eq!(event_u64(first, "writeback_tick"), raw_ready);
    assert_eq!(
        event_u64(second, "writeback_tick"),
        raw_ready + u64::from(writeback_width == 1)
    );
    assert_eq!(
        event_u64(div, "writeback_tick"),
        raw_ready + if writeback_width == 1 { 2 } else { 1 }
    );
}
fn assert_oldest_first_commit(json: &Value) {
    let events = PAIR_PCS.map(|pc| event_at_pc(json, pc));
    assert_event_order([events[0], events[1], events[2]], "sequence", true);
    assert!(event_u64(events[2], "sequence") < event_u64(events[3], "sequence"));
    assert!(events
        .windows(2)
        .all(|pair| event_u64(pair[0], "commit_tick") <= event_u64(pair[1], "commit_tick")));
}
fn assert_final_witness(json: &Value) {
    assert_register(json, "x3", "0x2a");
    assert_register(json, "x11", "0x11");
    assert_register(json, "x12", "0x33");
    assert_register(json, "x13", "0x34");
    assert_eq!(
        page_witness_hex(json, FIRST_PHYSICAL_PAGE).as_deref(),
        Some("110000000000000011000000000000002a00000000000000")
    );
    assert_eq!(
        page_witness_hex(json, SECOND_PHYSICAL_PAGE).as_deref(),
        Some("330000000000000033000000000000003400000000000000")
    );
}
fn page_witness_hex(json: &Value, page: u64) -> Option<String> {
    [page, page + 16]
        .into_iter()
        .map(|address| memory_dump_hex(json, address))
        .collect::<Option<Vec<_>>>()
        .map(|chunks| chunks.concat())
}
fn fetch_record_at_pc<'a>(json: &'a Value, pc: &str) -> &'a Value {
    let records = json
        .pointer("/debug/fetch_trace")
        .and_then(Value::as_array)
        .expect("translated pair Fetch trace")
        .iter()
        .filter(|record| record.pointer("/pc").and_then(Value::as_str) == Some(pc))
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "exact translated Fetch trace at {pc}");
    records[0]
}
fn fetch_request_identity(json: &Value, fetch: &Value) -> RequestIdentity {
    let sequence = event_u64(fetch, "sequence");
    let records = memory_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "channel") == "fetch"
                && event_str(record, "kind") == "request_sent"
                && event_u64(record, "request") == sequence
        })
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "fetch request identity for {fetch}");
    request_identity(records[0])
}
fn data_request_sent_records(json: &Value) -> Vec<&Value> {
    memory_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "channel") == "data" && event_str(record, "kind") == "request_sent"
        })
        .collect()
}
fn pair_request_identities(json: &Value) -> [PcRequestIdentity; 2] {
    let events = [
        memory_result_event_at_pc(json, FIRST_PC),
        memory_result_event_at_pc(json, SECOND_PC),
    ];
    assert!(event_u64(events[0], "issue_tick") < event_u64(events[1], "issue_tick"));
    std::array::from_fn(|index| {
        let issue_tick = event_u64(events[index], "issue_tick");
        let sent = data_request_sent_records(json)
            .into_iter()
            .filter(|record| event_u64(record, "tick") == issue_tick)
            .collect::<Vec<_>>();
        assert_eq!(
            sent.len(),
            1,
            "identity control request at {}",
            PAIR_PCS[index]
        );
        let data = request_identity(sent[0]);
        let response = data_record_for_identity(json, "response_arrived", data).unwrap();
        assert_eq!(
            event_u64(response, "tick"),
            event_u64(events[index], "lsq_data_response_tick")
        );
        PcRequestIdentity {
            data,
            fetch: fetch_request_identity(json, fetch_record_at_pc(json, PAIR_PCS[index])),
        }
    })
}
fn pair_request_evidence<'a>(
    json: &'a Value,
    identities: &[PcRequestIdentity; 2],
) -> [PairRequestEvidence<'a>; 2] {
    let specs = [
        (FIRST_PC, FIRST_PHYSICAL_PAGE),
        (SECOND_PC, SECOND_PHYSICAL_PAGE),
    ];
    let events = specs.map(|(pc, _)| memory_result_event_at_pc(json, pc));
    assert!(event_u64(events[0], "sequence") < event_u64(events[1], "sequence"));
    std::array::from_fn(|index| {
        let (pc, physical_address) = specs[index];
        let event = events[index];
        assert_data_completion_at_pc(json, event, pc, physical_address);
        let identity = identities[index].data;
        let sent = request_sent_for_identity(json, identity)
            .unwrap_or_else(|| panic!("missing main request for {pc} identity {identity:?}"));
        let response = data_record_for_identity(json, "response_arrived", identity)
            .unwrap_or_else(|| panic!("missing response for {pc} identity {identity:?}"));
        assert_eq!(event_u64(sent, "tick"), event_u64(event, "issue_tick"));
        assert_eq!(
            event_u64(response, "tick"),
            event_u64(event, "lsq_data_response_tick")
        );
        PairRequestEvidence {
            pc,
            physical_address,
            event,
            sent,
            response,
            identity,
        }
    })
}
fn assert_data_completion_at_pc(json: &Value, event: &Value, pc: &str, address: u64) {
    let address = format!("0x{address:x}");
    let records = data_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "kind") == "load"
                && event_str(record, "address") == address
                && event_u64(record, "size") == 8
        })
        .collect::<Vec<_>>();
    assert_eq!(records.len(), 1, "exact translated Data completion at {pc}");
    let tick = event_u64(records[0], "tick");
    assert_eq!(tick, event_u64(event, "lsq_data_response_tick"));
}
fn request_sent_for_identity(json: &Value, identity: RequestIdentity) -> Option<&Value> {
    data_record_for_identity(json, "request_sent", identity)
}
fn data_record_for_identity<'a>(
    json: &'a Value,
    kind: &str,
    identity: RequestIdentity,
) -> Option<&'a Value> {
    let records = memory_trace(json)
        .iter()
        .filter(|record| {
            event_str(record, "channel") == "data"
                && event_str(record, "kind") == kind
                && request_identity(record) == identity
        })
        .collect::<Vec<_>>();
    assert!(records.len() <= 1, "duplicate {kind} for {identity:?}");
    records.into_iter().next()
}
fn pair_request_trace(json: &Value) -> String {
    data_request_sent_records(json)
        .into_iter()
        .map(|record| {
            format!(
                "{:?}@{}",
                request_identity(record),
                event_u64(record, "tick")
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}
fn memory_trace(json: &Value) -> &[Value] {
    json.pointer("/debug/memory_trace")
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_else(|| panic!("translated pair Memory trace missing: {json}"))
}
fn request_identity(record: &Value) -> RequestIdentity {
    RequestIdentity {
        agent: event_u64(record, "request_agent"),
        sequence: event_u64(record, "request"),
    }
}
