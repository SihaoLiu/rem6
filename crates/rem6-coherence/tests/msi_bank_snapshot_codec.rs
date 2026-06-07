use rem6_coherence::MsiBankDirectoryHarnessSnapshot;

const FORMAT_VERSION: u64 = 5;
const LINE_BYTES: u64 = 16;
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const MEMORY_REQUEST_MIN_RECORD_BYTES: usize =
    U32_BYTES + U64_BYTES + U8_BYTES + U64_BYTES * 3 + U8_BYTES * 4;
const MSHR_TARGET_MIN_RECORD_BYTES: usize =
    MEMORY_REQUEST_MIN_RECORD_BYTES + U64_BYTES * 2 + U8_BYTES * 3;

fn write_u8(payload: &mut Vec<u8>, value: u8) {
    payload.push(value);
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn payload_header() -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, FORMAT_VERSION);
    write_u64(&mut payload, LINE_BYTES);
    payload
}

fn payload_after_cache_count() -> Vec<u8> {
    let mut payload = payload_header();
    write_u64(&mut payload, 0);
    payload
}

fn write_empty_top_level_counts_before_cpu_responses(payload: &mut Vec<u8>) {
    write_u64(payload, 0);
    write_u64(payload, 0);
}

fn write_empty_top_level_counts_before_directory_decisions(payload: &mut Vec<u8>) {
    write_empty_top_level_counts_before_cpu_responses(payload);
    write_u64(payload, 0);
}

fn write_empty_top_level_counts_before_parallel_cycles(payload: &mut Vec<u8>) {
    write_empty_top_level_counts_before_directory_decisions(payload);
    write_u64(payload, 0);
}

fn write_cache_bank_header_until_line_count(payload: &mut Vec<u8>) {
    write_u32(payload, 1);
    write_u64(payload, LINE_BYTES);
    write_u64(payload, 0);
}

fn payload_with_one_cache_bank() -> Vec<u8> {
    let mut payload = payload_header();
    write_u64(&mut payload, 1);
    write_cache_bank_header_until_line_count(&mut payload);
    payload
}

fn write_cache_controller_until_trace_count(payload: &mut Vec<u8>) {
    write_u32(payload, 1);
    write_u64(payload, 0x1000);
    write_u8(payload, 0);
    write_u64(payload, LINE_BYTES);
    write_u64(payload, 0);
    write_u8(payload, 0);
}

fn write_cache_bank_with_no_lines_until_mshr_entry_count(payload: &mut Vec<u8>) {
    write_cache_bank_header_until_line_count(payload);
    write_u64(payload, 0);
    write_u8(payload, 1);
    write_u64(payload, 1);
    write_u64(payload, 1);
    write_u64(payload, 0);
    write_u64(payload, 0);
    write_u64(payload, 0);
}

fn payload_with_one_cache_bank_until_mshr_entry_count() -> Vec<u8> {
    let mut payload = payload_header();
    write_u64(&mut payload, 1);
    write_cache_bank_with_no_lines_until_mshr_entry_count(&mut payload);
    payload
}

fn write_mshr_entry_until_target_count(payload: &mut Vec<u8>) {
    write_u64(payload, 0);
    write_u64(payload, 0x1000);
    write_u64(payload, 0);
    write_u64(payload, 0);
    write_u8(payload, 0);
    write_u8(payload, 0);
}

fn write_mshr_target_with_request_payload(
    payload: &mut Vec<u8>,
    operation: u8,
    data: Option<&[u8]>,
    byte_mask: Option<&[bool]>,
) {
    write_u32(payload, 1);
    write_u64(payload, 10);
    write_u8(payload, operation);
    write_u64(payload, 0x1000);
    write_u64(payload, 8);
    write_u64(payload, LINE_BYTES);
    match data {
        Some(data) => {
            write_u8(payload, 1);
            write_u64(payload, data.len() as u64);
            payload.extend_from_slice(data);
        }
        None => write_u8(payload, 0),
    }
    match byte_mask {
        Some(bits) => {
            write_u8(payload, 1);
            write_u64(payload, bits.len() as u64);
            for bit in bits {
                write_u8(payload, u8::from(*bit));
            }
        }
        None => write_u8(payload, 0),
    }
    write_u8(payload, 0);
    write_u8(payload, 0);

    write_u64(payload, 0);
    write_u64(payload, 0);
    write_u8(payload, 0);
    write_u8(payload, 0);
    write_u8(payload, 0);
}

fn payload_with_one_mshr_target_request(
    operation: u8,
    data: Option<&[u8]>,
    byte_mask: Option<&[bool]>,
) -> Vec<u8> {
    let mut payload = payload_with_one_cache_bank_until_mshr_entry_count();
    write_u64(&mut payload, 1);
    write_mshr_entry_until_target_count(&mut payload);
    write_u64(&mut payload, 1);
    write_mshr_target_with_request_payload(&mut payload, operation, data, byte_mask);
    write_empty_top_level_counts_before_parallel_cycles(&mut payload);
    write_u64(&mut payload, 0);
    payload
}

fn write_directory_state_without_owner_or_sharers(payload: &mut Vec<u8>) {
    write_u64(payload, 0x1000);
    write_u8(payload, 0);
    write_u64(payload, 0);
}

fn write_directory_decision_until_snoop_count(payload: &mut Vec<u8>) {
    write_u64(payload, 0x1000);
    write_u32(payload, 1);
    write_u64(payload, 10);
    write_directory_state_without_owner_or_sharers(payload);
    write_directory_state_without_owner_or_sharers(payload);
}

fn pad_to_mshr_target_min(payload: &mut Vec<u8>, target_start: usize) -> usize {
    let written = payload.len() - target_start;
    let padding = MSHR_TARGET_MIN_RECORD_BYTES.saturating_sub(written);
    payload.resize(payload.len() + padding, 0);
    padding
}

#[test]
fn msi_bank_snapshot_rejects_impossible_directory_line_count() {
    let mut payload = payload_after_cache_count();
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI directory line count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_backing_line_count() {
    let mut payload = payload_after_cache_count();
    write_u64(&mut payload, 0);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI backing line count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_directory_sharer_count() {
    let mut payload = payload_after_cache_count();
    write_u64(&mut payload, 1);
    write_u64(&mut payload, 0x1000);
    write_u8(&mut payload, 0);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI directory sharer count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_cpu_response_count() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_cpu_responses(&mut payload);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI CPU response count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_cpu_response_count_with_missing_optional_data_flag() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_cpu_responses(&mut payload);
    write_u64(&mut payload, 1);
    write_u64(&mut payload, 0);
    write_u8(&mut payload, 0);
    write_u32(&mut payload, 1);
    write_u64(&mut payload, 10);
    write_u8(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI CPU response count 1 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_directory_decision_count() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_directory_decisions(&mut payload);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI directory decision count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_directory_decision_count_with_missing_grant_flag() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_directory_decisions(&mut payload);
    write_u64(&mut payload, 1);
    write_directory_decision_until_snoop_count(&mut payload);
    write_u64(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI directory decision count 1 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_parallel_cycle_count() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_parallel_cycles(&mut payload);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI parallel cycle count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_cache_bank_line_count_with_missing_pending_flag() {
    let mut payload = payload_with_one_cache_bank();
    write_u64(&mut payload, 1);
    write_cache_controller_until_trace_count(&mut payload);
    write_u64(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI cache bank line count 1 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_cache_bank_line_count() {
    let mut payload = payload_with_one_cache_bank();
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI cache bank line count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_cache_line_trace_length() {
    let mut payload = payload_with_one_cache_bank();
    write_u64(&mut payload, 1);
    write_cache_controller_until_trace_count(&mut payload);
    write_u64(&mut payload, u64::MAX);
    write_u8(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI cache line trace length 18446744073709551615 exceeds remaining payload capacity 1 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_mshr_entry_count() {
    let mut payload = payload_with_one_cache_bank_until_mshr_entry_count();
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI cache bank MSHR entry count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_mshr_target_count() {
    let mut payload = payload_with_one_cache_bank_until_mshr_entry_count();
    write_u64(&mut payload, 1);
    write_mshr_entry_until_target_count(&mut payload);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI cache bank MSHR target count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_directory_decision_snoop_count() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_directory_decisions(&mut payload);
    write_u64(&mut payload, 1);
    write_directory_decision_until_snoop_count(&mut payload);
    write_u64(&mut payload, u64::MAX);
    write_u8(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI decision snoop count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_parallel_cycle_accepted_count() {
    let mut payload = payload_after_cache_count();
    write_empty_top_level_counts_before_parallel_cycles(&mut payload);
    write_u64(&mut payload, 1);
    write_u64(&mut payload, 0);
    write_u64(&mut payload, 0);
    write_u64(&mut payload, u64::MAX);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI parallel cycle accepted count 18446744073709551615 exceeds remaining payload capacity 0 records"
    );
}

#[test]
fn msi_bank_snapshot_rejects_impossible_request_byte_mask_length() {
    let mut payload = payload_with_one_cache_bank_until_mshr_entry_count();
    write_u64(&mut payload, 1);
    write_mshr_entry_until_target_count(&mut payload);
    write_u64(&mut payload, 1);
    let target_start = payload.len();
    write_u32(&mut payload, 1);
    write_u64(&mut payload, 10);
    write_u8(&mut payload, 1);
    write_u64(&mut payload, 0x1000);
    write_u64(&mut payload, 8);
    write_u64(&mut payload, LINE_BYTES);
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 1);
    write_u64(&mut payload, u64::MAX);
    let capacity = pad_to_mshr_target_min(&mut payload, target_start);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        format!(
            "MSI request byte mask length 18446744073709551615 exceeds remaining payload capacity {capacity} records"
        )
    );
}

#[test]
fn msi_bank_snapshot_rejects_locked_rmw_read_request_with_data() {
    let payload = payload_with_one_mshr_target_request(15, Some(&[0x5a; 8]), None);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI locked RMW read request cannot carry data");
}

#[test]
fn msi_bank_snapshot_rejects_locked_rmw_read_request_with_byte_mask() {
    let payload = payload_with_one_mshr_target_request(15, None, Some(&[true; 8]));

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI locked RMW read request cannot carry a byte mask"
    );
}

#[test]
fn msi_bank_snapshot_rejects_load_locked_request_with_data() {
    let payload = payload_with_one_mshr_target_request(17, Some(&[0x6b; 8]), None);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI load locked request cannot carry data");
}

#[test]
fn msi_bank_snapshot_rejects_load_locked_request_with_byte_mask() {
    let payload = payload_with_one_mshr_target_request(17, None, Some(&[true; 8]));

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI load locked request cannot carry a byte mask");
}

#[test]
fn msi_bank_snapshot_rejects_store_conditional_request_without_data() {
    let payload = payload_with_one_mshr_target_request(18, None, Some(&[true; 8]));

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI store conditional request is missing data");
}

#[test]
fn msi_bank_snapshot_rejects_store_conditional_request_without_byte_mask() {
    let payload = payload_with_one_mshr_target_request(18, Some(&[0x7c; 8]), None);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI store conditional request is missing byte mask");
}

#[test]
fn msi_bank_snapshot_rejects_atomic_no_return_request_rebuild() {
    let payload = payload_with_one_mshr_target_request(24, Some(&[0x8d; 8]), Some(&[true; 8]));

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(
        error,
        "MSI checkpoint decoder cannot rebuild AtomicNoReturn requests"
    );
}

#[test]
fn msi_bank_snapshot_rejects_write_clean_request_with_byte_mask() {
    let mut payload = payload_with_one_cache_bank_until_mshr_entry_count();
    write_u64(&mut payload, 1);
    write_mshr_entry_until_target_count(&mut payload);
    write_u64(&mut payload, 1);

    write_u32(&mut payload, 1);
    write_u64(&mut payload, 10);
    write_u8(&mut payload, 12);
    write_u64(&mut payload, 0x1000);
    write_u64(&mut payload, LINE_BYTES);
    write_u64(&mut payload, LINE_BYTES);
    write_u8(&mut payload, 1);
    write_u64(&mut payload, LINE_BYTES);
    payload.extend(std::iter::repeat_n(0x5a, LINE_BYTES as usize));
    write_u8(&mut payload, 1);
    write_u64(&mut payload, LINE_BYTES);
    for _ in 0..LINE_BYTES {
        write_u8(&mut payload, 1);
    }
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 0);

    write_u64(&mut payload, 0);
    write_u64(&mut payload, 0);
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 0);

    write_empty_top_level_counts_before_parallel_cycles(&mut payload);
    write_u64(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI write clean request cannot carry a byte mask");
}

#[test]
fn msi_bank_snapshot_rejects_clean_shared_request_with_byte_mask() {
    let mut payload = payload_with_one_cache_bank_until_mshr_entry_count();
    write_u64(&mut payload, 1);
    write_mshr_entry_until_target_count(&mut payload);
    write_u64(&mut payload, 1);

    write_u32(&mut payload, 1);
    write_u64(&mut payload, 11);
    write_u8(&mut payload, 13);
    write_u64(&mut payload, 0x1000);
    write_u64(&mut payload, LINE_BYTES);
    write_u64(&mut payload, LINE_BYTES);
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 1);
    write_u64(&mut payload, LINE_BYTES);
    for _ in 0..LINE_BYTES {
        write_u8(&mut payload, 1);
    }
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 0);

    write_u64(&mut payload, 0);
    write_u64(&mut payload, 0);
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 0);
    write_u8(&mut payload, 0);

    write_empty_top_level_counts_before_parallel_cycles(&mut payload);
    write_u64(&mut payload, 0);

    let error = MsiBankDirectoryHarnessSnapshot::from_bytes(&payload).unwrap_err();

    assert_eq!(error, "MSI clean shared request cannot carry a byte mask");
}
