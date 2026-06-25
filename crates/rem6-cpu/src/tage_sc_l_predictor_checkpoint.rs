use crate::loop_predictor::{
    LoopBranchPredictorConfig, LoopBranchPredictorSnapshot, LoopEntrySnapshot,
};
use crate::ltage_predictor::{LTageBranchPredictorConfig, LTageBranchPredictorSnapshot};
use crate::statistical_corrector::{
    StatisticalCorrectorConfig, StatisticalCorrectorSnapshot, StatisticalCorrectorThreadSnapshot,
};
use crate::tage_predictor::{
    FoldedHistorySnapshot, TageBranchPredictorConfig, TageBranchPredictorSnapshot, TageTableEntry,
    TageThreadSnapshot,
};
use crate::{TageScLBranchPredictor, TageScLBranchPredictorError, TageScLBranchPredictorSnapshot};

const CHECKPOINT_MAGIC: [u8; 4] = *b"RTSL";
const CHECKPOINT_VERSION: u8 = 1;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const MIN_CHECKPOINT_BYTES: usize = CHECKPOINT_MAGIC.len() + 1;
const MAX_SC_HISTORY_LENGTHS: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TageScLBranchPredictorCheckpointPayload {
    snapshot: TageScLBranchPredictorSnapshot,
}

impl TageScLBranchPredictorCheckpointPayload {
    pub fn from_snapshot(
        snapshot: TageScLBranchPredictorSnapshot,
    ) -> Result<Self, TageScLBranchPredictorError> {
        validate_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub const fn snapshot(&self) -> &TageScLBranchPredictorSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> TageScLBranchPredictorSnapshot {
        self.snapshot
    }

    pub fn encode(&self) -> Vec<u8> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        push_u8(&mut payload, CHECKPOINT_VERSION);

        encode_tage_config(&mut payload, &self.snapshot.config.ltage.tage)
            .expect("validated TAGE-SC-L checkpoint TAGE config is encodable");
        encode_loop_config(&mut payload, &self.snapshot.config.ltage.loop_predictor)
            .expect("validated TAGE-SC-L checkpoint loop config is encodable");
        encode_statistical_corrector_config(
            &mut payload,
            &self.snapshot.config.statistical_corrector,
        )
        .expect("validated TAGE-SC-L checkpoint statistical-corrector config is encodable");

        encode_ltage_snapshot(&mut payload, &self.snapshot.ltage)
            .expect("validated TAGE-SC-L checkpoint LTAGE state is encodable");
        encode_statistical_corrector_snapshot(&mut payload, &self.snapshot.statistical_corrector)
            .expect("validated TAGE-SC-L checkpoint statistical-corrector state is encodable");
        push_u64(&mut payload, self.snapshot.lookup_count);
        push_u64(&mut payload, self.snapshot.update_count);
        push_u64(&mut payload, self.snapshot.history_update_count);
        push_u64(&mut payload, self.snapshot.repair_count);

        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, TageScLBranchPredictorError> {
        if payload.len() < MIN_CHECKPOINT_BYTES {
            return Err(TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: MIN_CHECKPOINT_BYTES,
                actual: payload.len(),
            });
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(TageScLBranchPredictorError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != CHECKPOINT_VERSION {
            return Err(TageScLBranchPredictorError::UnsupportedCheckpointVersion { version });
        }

        let tage_config = decode_tage_config(payload, &mut offset)?;
        let loop_config = decode_loop_config(payload, &mut offset)?;
        let ltage_config = LTageBranchPredictorConfig::new(tage_config, loop_config)?;
        let statistical_corrector_config =
            decode_statistical_corrector_config(payload, &mut offset)?;
        let config =
            crate::TageScLBranchPredictorConfig::new(ltage_config, statistical_corrector_config)?;

        let ltage = decode_ltage_snapshot(payload, &mut offset, &config.ltage)?;
        let statistical_corrector = decode_statistical_corrector_snapshot(
            payload,
            &mut offset,
            &config.statistical_corrector,
        )?;
        let lookup_count = read_u64(payload, &mut offset)?;
        let update_count = read_u64(payload, &mut offset)?;
        let history_update_count = read_u64(payload, &mut offset)?;
        let repair_count = read_u64(payload, &mut offset)?;

        if offset != payload.len() {
            return Err(TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: offset,
                actual: payload.len(),
            });
        }

        Self::from_snapshot(TageScLBranchPredictorSnapshot::from_parts(
            config,
            ltage,
            statistical_corrector,
            lookup_count,
            update_count,
            history_update_count,
            repair_count,
        ))
    }
}

fn validate_snapshot(
    snapshot: &TageScLBranchPredictorSnapshot,
) -> Result<(), TageScLBranchPredictorError> {
    let mut predictor = TageScLBranchPredictor::new(snapshot.config.clone());
    predictor.restore(snapshot)
}

fn encode_ltage_snapshot(
    payload: &mut Vec<u8>,
    snapshot: &LTageBranchPredictorSnapshot,
) -> Result<(), TageScLBranchPredictorError> {
    encode_tage_snapshot(payload, &snapshot.tage)?;
    encode_loop_snapshot(payload, &snapshot.loop_predictor)?;
    push_u64(payload, snapshot.lookup_count);
    push_u64(payload, snapshot.update_count);
    push_u64(payload, snapshot.repair_count);
    Ok(())
}

fn decode_ltage_snapshot(
    payload: &[u8],
    offset: &mut usize,
    config: &LTageBranchPredictorConfig,
) -> Result<LTageBranchPredictorSnapshot, TageScLBranchPredictorError> {
    let tage = decode_tage_snapshot(payload, offset, &config.tage)?;
    let loop_predictor = decode_loop_snapshot(payload, offset, &config.loop_predictor)?;
    let lookup_count = read_u64(payload, offset)?;
    let update_count = read_u64(payload, offset)?;
    let repair_count = read_u64(payload, offset)?;
    Ok(LTageBranchPredictorSnapshot::from_parts(
        config.clone(),
        tage,
        loop_predictor,
        lookup_count,
        update_count,
        repair_count,
    ))
}

fn encode_tage_config(
    payload: &mut Vec<u8>,
    config: &TageBranchPredictorConfig,
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, "tage-threads", config.threads)?;
    push_usize(payload, "tage-history-tables", config.history_tables)?;
    push_usize(payload, "tage-min-history", config.min_history)?;
    push_usize(payload, "tage-max-history", config.max_history)?;
    push_u8_vec(payload, "tage-tag-widths", &config.tag_widths)?;
    push_u8_vec(payload, "tage-log-table-sizes", &config.log_table_sizes)?;
    push_u8(payload, config.log_ratio_bimodal_hysteresis);
    push_u8(payload, config.counter_bits);
    push_u8(payload, config.useful_bits);
    push_u8(payload, config.path_history_bits);
    push_u8(payload, config.log_useful_reset_period);
    push_usize(
        payload,
        "tage-use-alt-counters",
        config.use_alt_on_new_counters,
    )?;
    push_u8(payload, config.use_alt_on_new_bits);
    push_usize(payload, "tage-max-allocations", config.max_allocations)?;
    push_u8(payload, config.inst_shift);
    push_bool(payload, config.taken_only_history);
    push_bool(payload, config.speculative_history);
    Ok(())
}

fn decode_tage_config(
    payload: &[u8],
    offset: &mut usize,
) -> Result<TageBranchPredictorConfig, TageScLBranchPredictorError> {
    let threads = read_usize(payload, offset)?;
    let history_tables = read_usize(payload, offset)?;
    let min_history = read_usize(payload, offset)?;
    let max_history = read_usize(payload, offset)?;
    let expected_tables = checked_sum("tage-table-count", history_tables, 1)?;
    let tag_widths = read_expected_u8_vec("tage-tag-widths", payload, offset, expected_tables)?;
    let log_table_sizes =
        read_expected_u8_vec("tage-log-table-sizes", payload, offset, expected_tables)?;
    let log_ratio_bimodal_hysteresis = read_u8(payload, offset)?;
    let counter_bits = read_u8(payload, offset)?;
    let useful_bits = read_u8(payload, offset)?;
    let path_history_bits = read_u8(payload, offset)?;
    let log_useful_reset_period = read_u8(payload, offset)?;
    let use_alt_on_new_counters = read_usize(payload, offset)?;
    let use_alt_on_new_bits = read_u8(payload, offset)?;
    let max_allocations = read_usize(payload, offset)?;
    let inst_shift = read_u8(payload, offset)?;
    let taken_only_history = read_bool("tage-taken-only-history", payload, offset)?;
    let speculative_history = read_bool("tage-speculative-history", payload, offset)?;

    TageBranchPredictorConfig::with_options(
        threads,
        history_tables,
        min_history,
        max_history,
        tag_widths,
        log_table_sizes,
        log_ratio_bimodal_hysteresis,
        counter_bits,
        useful_bits,
        path_history_bits,
        log_useful_reset_period,
        use_alt_on_new_counters,
        use_alt_on_new_bits,
        max_allocations,
        inst_shift,
        taken_only_history,
        speculative_history,
    )
    .map_err(|error| {
        TageScLBranchPredictorError::LTage(crate::LTageBranchPredictorError::Tage(error))
    })
}

fn encode_tage_snapshot(
    payload: &mut Vec<u8>,
    snapshot: &TageBranchPredictorSnapshot,
) -> Result<(), TageScLBranchPredictorError> {
    push_bool_vec(
        payload,
        "tage-bimodal-prediction",
        &snapshot.bimodal_prediction,
    )?;
    push_bool_vec(
        payload,
        "tage-bimodal-hysteresis",
        &snapshot.bimodal_hysteresis,
    )?;
    push_usize(
        payload,
        "tage-tagged-table-count",
        snapshot.tagged_tables.len(),
    )?;
    for table in &snapshot.tagged_tables {
        push_usize(payload, "tage-tagged-table-entries", table.len())?;
        for entry in table {
            push_i8(payload, entry.counter);
            push_u16(payload, entry.tag);
            push_u8(payload, entry.useful);
        }
    }
    push_usize(payload, "tage-thread-count", snapshot.threads.len())?;
    for thread in &snapshot.threads {
        encode_tage_thread(payload, thread)?;
    }
    push_i8_vec(
        payload,
        "tage-use-alt-on-new-counters",
        &snapshot.use_alt_on_new_counters,
    )?;
    push_u64(payload, snapshot.t_counter);
    push_u64(payload, snapshot.lookup_count);
    push_u64(payload, snapshot.update_count);
    push_u64(payload, snapshot.history_update_count);
    Ok(())
}

fn decode_tage_snapshot(
    payload: &[u8],
    offset: &mut usize,
    config: &TageBranchPredictorConfig,
) -> Result<TageBranchPredictorSnapshot, TageScLBranchPredictorError> {
    let bimodal_prediction = read_expected_bool_vec(
        "tage-bimodal-prediction",
        payload,
        offset,
        1usize << config.log_table_sizes[0],
    )?;
    let expected_hysteresis =
        ((1usize << config.log_table_sizes[0]) >> config.log_ratio_bimodal_hysteresis).max(1);
    let bimodal_hysteresis = read_expected_bool_vec(
        "tage-bimodal-hysteresis",
        payload,
        offset,
        expected_hysteresis,
    )?;

    let table_count = read_usize(payload, offset)?;
    expect_len(
        "tage-tagged-table-count",
        config.history_tables + 1,
        table_count,
    )?;
    let mut tagged_tables = Vec::with_capacity(table_count);
    for bank in 0..table_count {
        let entries = read_usize(payload, offset)?;
        let expected_entries = if bank == 0 {
            0
        } else {
            1usize << config.log_table_sizes[bank]
        };
        expect_len("tage-tagged-table-entries", expected_entries, entries)?;
        let mut table = Vec::with_capacity(entries);
        for _ in 0..entries {
            let counter = read_i8(payload, offset)?;
            let tag = read_u16(payload, offset)?;
            let useful = read_u8(payload, offset)?;
            if bank != 0 {
                check_signed_counter(
                    "tage-tagged-counter",
                    i64::from(counter),
                    config.counter_bits,
                )?;
                check_unsigned_counter("tage-tagged-tag", u64::from(tag), config.tag_widths[bank])?;
                check_unsigned_counter(
                    "tage-tagged-useful",
                    u64::from(useful),
                    config.useful_bits,
                )?;
            }
            table.push(TageTableEntry {
                counter,
                tag,
                useful,
            });
        }
        tagged_tables.push(table);
    }

    let thread_count = read_usize(payload, offset)?;
    expect_len("tage-thread-count", config.threads, thread_count)?;
    let mut threads = Vec::with_capacity(thread_count);
    for _ in 0..thread_count {
        threads.push(decode_tage_thread(payload, offset, config)?);
    }
    let use_alt_on_new_counters = read_expected_i8_vec(
        "tage-use-alt-on-new-counters",
        payload,
        offset,
        config.use_alt_on_new_counters,
    )?;
    for counter in &use_alt_on_new_counters {
        check_signed_counter(
            "tage-use-alt-on-new-counter",
            i64::from(*counter),
            config.use_alt_on_new_bits,
        )?;
    }
    let t_counter = read_u64(payload, offset)?;
    let lookup_count = read_u64(payload, offset)?;
    let update_count = read_u64(payload, offset)?;
    let history_update_count = read_u64(payload, offset)?;

    Ok(TageBranchPredictorSnapshot::from_parts(
        config.clone(),
        bimodal_prediction,
        bimodal_hysteresis,
        tagged_tables,
        threads,
        use_alt_on_new_counters,
        t_counter,
        lookup_count,
        update_count,
        history_update_count,
    ))
}

fn encode_tage_thread(
    payload: &mut Vec<u8>,
    thread: &TageThreadSnapshot,
) -> Result<(), TageScLBranchPredictorError> {
    push_u32(payload, thread.path_history);
    push_u32(payload, thread.non_spec_path_history);
    push_u8_vec(payload, "tage-global-history", &thread.global_history)?;
    push_folded_history_vec(payload, "tage-compute-indices", &thread.compute_indices)?;
    push_usize(
        payload,
        "tage-compute-tag-groups",
        thread.compute_tags.len(),
    )?;
    for tags in &thread.compute_tags {
        push_folded_history_vec(payload, "tage-compute-tags", tags)?;
    }
    Ok(())
}

fn decode_tage_thread(
    payload: &[u8],
    offset: &mut usize,
    config: &TageBranchPredictorConfig,
) -> Result<TageThreadSnapshot, TageScLBranchPredictorError> {
    let path_history = read_u32(payload, offset)?;
    let non_spec_path_history = read_u32(payload, offset)?;
    check_unsigned_counter(
        "tage-path-history",
        u64::from(path_history),
        config.path_history_bits,
    )?;
    check_unsigned_counter(
        "tage-non-spec-path-history",
        u64::from(non_spec_path_history),
        config.path_history_bits,
    )?;
    let global_history = read_expected_u8_vec(
        "tage-global-history",
        payload,
        offset,
        config.max_history + 1,
    )?;
    for value in &global_history {
        if *value > 1 {
            return Err(TageScLBranchPredictorError::InvalidCheckpointBool {
                name: "tage-global-history",
                value: *value,
            });
        }
    }
    let compute_indices = read_expected_folded_history_vec(
        "tage-compute-indices",
        payload,
        offset,
        config.history_tables + 1,
    )?;
    validate_folded_history("tage-compute-indices", &compute_indices[0], 1, 1)?;
    for bank in 1..=config.history_tables {
        validate_folded_history(
            "tage-compute-indices",
            &compute_indices[bank],
            config.history_lengths[bank],
            config.log_table_sizes[bank],
        )?;
    }
    let tag_group_count = read_usize(payload, offset)?;
    expect_len("tage-compute-tag-groups", 2, tag_group_count)?;
    let compute_tags0 = read_expected_folded_history_vec(
        "tage-compute-tags",
        payload,
        offset,
        config.history_tables + 1,
    )?;
    validate_folded_history("tage-compute-tags", &compute_tags0[0], 1, 1)?;
    for bank in 1..=config.history_tables {
        validate_folded_history(
            "tage-compute-tags",
            &compute_tags0[bank],
            config.history_lengths[bank],
            config.tag_widths[bank],
        )?;
    }
    let compute_tags1 = read_expected_folded_history_vec(
        "tage-compute-tags",
        payload,
        offset,
        config.history_tables + 1,
    )?;
    validate_folded_history("tage-compute-tags", &compute_tags1[0], 1, 1)?;
    for bank in 1..=config.history_tables {
        validate_folded_history(
            "tage-compute-tags",
            &compute_tags1[bank],
            config.history_lengths[bank],
            config.tag_widths[bank] - 1,
        )?;
    }

    Ok(TageThreadSnapshot::from_parts(
        path_history,
        non_spec_path_history,
        global_history,
        compute_indices,
        [compute_tags0, compute_tags1],
    ))
}

fn push_folded_history_vec(
    payload: &mut Vec<u8>,
    name: &'static str,
    histories: &[FoldedHistorySnapshot],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, histories.len())?;
    for history in histories {
        push_usize(
            payload,
            "folded-history-original-bits",
            history.original_bits,
        )?;
        push_u8(payload, history.compressed_bits);
        push_u32(payload, history.compressed);
    }
    Ok(())
}

fn read_expected_folded_history_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected: usize,
) -> Result<Vec<FoldedHistorySnapshot>, TageScLBranchPredictorError> {
    let len = read_usize(payload, offset)?;
    expect_len(name, expected, len)?;
    let mut histories = Vec::with_capacity(len);
    for _ in 0..len {
        let original_bits = read_usize(payload, offset)?;
        let compressed_bits = read_u8(payload, offset)?;
        if compressed_bits == 0 {
            return Err(TageScLBranchPredictorError::InvalidCheckpointVectorLength {
                name: "folded-history-compressed-bits",
                expected: 1,
                actual: 0,
            });
        }
        let compressed = read_u32(payload, offset)?;
        check_unsigned_counter(
            "folded-history-compressed",
            u64::from(compressed),
            compressed_bits,
        )?;
        histories.push(FoldedHistorySnapshot::from_compressed(
            original_bits,
            compressed_bits,
            compressed,
        ));
    }
    Ok(histories)
}

fn validate_folded_history(
    name: &'static str,
    history: &FoldedHistorySnapshot,
    expected_original_bits: usize,
    expected_compressed_bits: u8,
) -> Result<(), TageScLBranchPredictorError> {
    expect_len(name, expected_original_bits, history.original_bits)?;
    expect_len(
        name,
        usize::from(expected_compressed_bits),
        usize::from(history.compressed_bits),
    )
}

fn encode_loop_config(
    payload: &mut Vec<u8>,
    config: &LoopBranchPredictorConfig,
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, "loop-threads", config.threads)?;
    push_u8(payload, config.log_size);
    push_u8(payload, config.log_assoc);
    push_u8(payload, config.age_bits);
    push_u8(payload, config.confidence_bits);
    push_u8(payload, config.tag_bits);
    push_u8(payload, config.iter_bits);
    push_u8(payload, config.with_loop_bits);
    push_u8(payload, config.inst_shift);
    push_bool(payload, config.use_direction_bit);
    push_bool(payload, config.use_speculation);
    push_bool(payload, config.use_hashing);
    push_bool(payload, config.restrict_allocation);
    push_u16(payload, config.initial_iter);
    push_u8(payload, config.initial_age);
    push_bool(payload, config.optional_age_reset);
    Ok(())
}

fn decode_loop_config(
    payload: &[u8],
    offset: &mut usize,
) -> Result<LoopBranchPredictorConfig, TageScLBranchPredictorError> {
    let threads = read_usize(payload, offset)?;
    let log_size = read_u8(payload, offset)?;
    let log_assoc = read_u8(payload, offset)?;
    let age_bits = read_u8(payload, offset)?;
    let confidence_bits = read_u8(payload, offset)?;
    let tag_bits = read_u8(payload, offset)?;
    let iter_bits = read_u8(payload, offset)?;
    let with_loop_bits = read_u8(payload, offset)?;
    let inst_shift = read_u8(payload, offset)?;
    let use_direction_bit = read_bool("loop-use-direction-bit", payload, offset)?;
    let use_speculation = read_bool("loop-use-speculation", payload, offset)?;
    let use_hashing = read_bool("loop-use-hashing", payload, offset)?;
    let restrict_allocation = read_bool("loop-restrict-allocation", payload, offset)?;
    let initial_iter = read_u16(payload, offset)?;
    let initial_age = read_u8(payload, offset)?;
    let optional_age_reset = read_bool("loop-optional-age-reset", payload, offset)?;

    LoopBranchPredictorConfig::with_options(
        threads,
        log_size,
        log_assoc,
        age_bits,
        confidence_bits,
        tag_bits,
        iter_bits,
        with_loop_bits,
        inst_shift,
        use_direction_bit,
        use_speculation,
        use_hashing,
        restrict_allocation,
        initial_iter,
        initial_age,
        optional_age_reset,
    )
    .map_err(|error| {
        TageScLBranchPredictorError::LTage(crate::LTageBranchPredictorError::Loop(error))
    })
}

fn encode_loop_snapshot(
    payload: &mut Vec<u8>,
    snapshot: &LoopBranchPredictorSnapshot,
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, "loop-entries", snapshot.entries.len())?;
    for entry in &snapshot.entries {
        push_u16(payload, entry.num_iter);
        push_u16(payload, entry.current_iter);
        push_u16(payload, entry.current_iter_spec);
        push_u8(payload, entry.confidence);
        push_u16(payload, entry.tag);
        push_u8(payload, entry.age);
        push_bool(payload, entry.direction);
    }
    push_usize(
        payload,
        "loop-allocation-cursors",
        snapshot.allocation_cursors.len(),
    )?;
    for cursor in &snapshot.allocation_cursors {
        push_usize(payload, "loop-allocation-cursor", *cursor)?;
    }
    push_i16(payload, snapshot.loop_use_counter);
    push_u64(payload, snapshot.lookup_count);
    push_u64(payload, snapshot.update_count);
    push_u64(payload, snapshot.squash_count);
    push_u64(payload, snapshot.used_count);
    push_u64(payload, snapshot.correct_count);
    push_u64(payload, snapshot.wrong_count);
    Ok(())
}

fn decode_loop_snapshot(
    payload: &[u8],
    offset: &mut usize,
    config: &LoopBranchPredictorConfig,
) -> Result<LoopBranchPredictorSnapshot, TageScLBranchPredictorError> {
    let entry_count = read_usize(payload, offset)?;
    expect_len("loop-entries", config.entries(), entry_count)?;
    let mut entries = Vec::with_capacity(entry_count);
    for _ in 0..entry_count {
        let num_iter = read_u16(payload, offset)?;
        let current_iter = read_u16(payload, offset)?;
        let current_iter_spec = read_u16(payload, offset)?;
        let confidence = read_u8(payload, offset)?;
        let tag = read_u16(payload, offset)?;
        let age = read_u8(payload, offset)?;
        let direction = read_bool("loop-entry-direction", payload, offset)?;
        check_unsigned_counter("loop-num-iter", u64::from(num_iter), config.iter_bits)?;
        check_unsigned_counter(
            "loop-current-iter",
            u64::from(current_iter),
            config.iter_bits,
        )?;
        check_unsigned_counter(
            "loop-current-iter-spec",
            u64::from(current_iter_spec),
            config.iter_bits,
        )?;
        check_unsigned_counter(
            "loop-confidence",
            u64::from(confidence),
            config.confidence_bits,
        )?;
        check_unsigned_counter("loop-tag", u64::from(tag), config.tag_bits)?;
        check_unsigned_counter("loop-age", u64::from(age), config.age_bits)?;
        entries.push(LoopEntrySnapshot::from_parts(
            num_iter,
            current_iter,
            current_iter_spec,
            confidence,
            tag,
            age,
            direction,
        ));
    }
    let cursor_count = read_usize(payload, offset)?;
    expect_len("loop-allocation-cursors", config.sets(), cursor_count)?;
    let mut allocation_cursors = Vec::with_capacity(cursor_count);
    for _ in 0..cursor_count {
        let cursor = read_usize(payload, offset)?;
        check_unsigned_value(
            "loop-allocation-cursor",
            cursor as u64,
            config.associativity() as u64 - 1,
        )?;
        allocation_cursors.push(cursor);
    }
    let loop_use_counter = read_i16(payload, offset)?;
    check_signed_counter(
        "loop-use-counter",
        i64::from(loop_use_counter),
        config.with_loop_bits,
    )?;
    let lookup_count = read_u64(payload, offset)?;
    let update_count = read_u64(payload, offset)?;
    let squash_count = read_u64(payload, offset)?;
    let used_count = read_u64(payload, offset)?;
    let correct_count = read_u64(payload, offset)?;
    let wrong_count = read_u64(payload, offset)?;

    Ok(LoopBranchPredictorSnapshot::from_parts(
        config.clone(),
        entries,
        allocation_cursors,
        loop_use_counter,
        lookup_count,
        update_count,
        squash_count,
        used_count,
        correct_count,
        wrong_count,
    ))
}

fn encode_statistical_corrector_config(
    payload: &mut Vec<u8>,
    config: &StatisticalCorrectorConfig,
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, "sc-threads", config.threads)?;
    push_u8(payload, config.log_bias);
    push_u8(payload, config.log_size_up);
    push_usize(
        payload,
        "sc-first-local-history-entries",
        config.num_entries_first_local_histories,
    )?;
    push_u8_vec(payload, "sc-global-lengths", &config.global_lengths)?;
    push_u8_vec(payload, "sc-backward-lengths", &config.backward_lengths)?;
    push_u8_vec(payload, "sc-local-lengths", &config.local_lengths)?;
    push_u8_vec(payload, "sc-imli-lengths", &config.imli_lengths)?;
    push_u8(payload, config.log_global);
    push_u8(payload, config.log_backward);
    push_u8(payload, config.log_local);
    push_u8(payload, config.log_imli);
    push_i8(payload, config.global_weight_init);
    push_i8(payload, config.backward_weight_init);
    push_i8(payload, config.local_weight_init);
    push_i8(payload, config.imli_weight_init);
    push_u8(payload, config.chooser_conf_width);
    push_u8(payload, config.update_threshold_width);
    push_u8(payload, config.per_pc_threshold_width);
    push_u8(payload, config.extra_weights_width);
    push_u8(payload, config.counter_width);
    push_i16(payload, config.initial_update_threshold);
    push_u8(payload, config.inst_shift);
    push_bool(payload, config.speculative_history);
    Ok(())
}

fn decode_statistical_corrector_config(
    payload: &[u8],
    offset: &mut usize,
) -> Result<StatisticalCorrectorConfig, TageScLBranchPredictorError> {
    let threads = read_usize(payload, offset)?;
    let log_bias = read_u8(payload, offset)?;
    let log_size_up = read_u8(payload, offset)?;
    let num_entries_first_local_histories = read_usize(payload, offset)?;
    let global_lengths =
        read_bounded_u8_vec("sc-global-lengths", payload, offset, MAX_SC_HISTORY_LENGTHS)?;
    let backward_lengths = read_bounded_u8_vec(
        "sc-backward-lengths",
        payload,
        offset,
        MAX_SC_HISTORY_LENGTHS,
    )?;
    let local_lengths =
        read_bounded_u8_vec("sc-local-lengths", payload, offset, MAX_SC_HISTORY_LENGTHS)?;
    let imli_lengths =
        read_bounded_u8_vec("sc-imli-lengths", payload, offset, MAX_SC_HISTORY_LENGTHS)?;
    let log_global = read_u8(payload, offset)?;
    let log_backward = read_u8(payload, offset)?;
    let log_local = read_u8(payload, offset)?;
    let log_imli = read_u8(payload, offset)?;
    let global_weight_init = read_i8(payload, offset)?;
    let backward_weight_init = read_i8(payload, offset)?;
    let local_weight_init = read_i8(payload, offset)?;
    let imli_weight_init = read_i8(payload, offset)?;
    let chooser_conf_width = read_u8(payload, offset)?;
    let update_threshold_width = read_u8(payload, offset)?;
    let per_pc_threshold_width = read_u8(payload, offset)?;
    let extra_weights_width = read_u8(payload, offset)?;
    let counter_width = read_u8(payload, offset)?;
    let initial_update_threshold = read_i16(payload, offset)?;
    let inst_shift = read_u8(payload, offset)?;
    let speculative_history = read_bool("sc-speculative-history", payload, offset)?;

    StatisticalCorrectorConfig::with_options(
        threads,
        log_bias,
        log_size_up,
        num_entries_first_local_histories,
        global_lengths,
        backward_lengths,
        local_lengths,
        imli_lengths,
        log_global,
        log_backward,
        log_local,
        log_imli,
        global_weight_init,
        backward_weight_init,
        local_weight_init,
        imli_weight_init,
        chooser_conf_width,
        update_threshold_width,
        per_pc_threshold_width,
        extra_weights_width,
        counter_width,
        initial_update_threshold,
        inst_shift,
        speculative_history,
    )
    .map_err(TageScLBranchPredictorError::StatisticalCorrector)
}

fn encode_statistical_corrector_snapshot(
    payload: &mut Vec<u8>,
    snapshot: &StatisticalCorrectorSnapshot,
) -> Result<(), TageScLBranchPredictorError> {
    push_i8_matrix(payload, "sc-global-gehl", &snapshot.global_gehl)?;
    push_i8_matrix(payload, "sc-backward-gehl", &snapshot.backward_gehl)?;
    push_i8_matrix(payload, "sc-local-gehl", &snapshot.local_gehl)?;
    push_i8_matrix(payload, "sc-imli-gehl", &snapshot.imli_gehl)?;
    push_i8_vec(payload, "sc-global-weights", &snapshot.global_weights)?;
    push_i8_vec(payload, "sc-backward-weights", &snapshot.backward_weights)?;
    push_i8_vec(payload, "sc-local-weights", &snapshot.local_weights)?;
    push_i8_vec(payload, "sc-imli-weights", &snapshot.imli_weights)?;
    push_i8_vec(payload, "sc-bias", &snapshot.bias)?;
    push_i8_vec(payload, "sc-bias-sk", &snapshot.bias_sk)?;
    push_i8_vec(payload, "sc-bias-bank", &snapshot.bias_bank)?;
    push_i8_vec(payload, "sc-bias-weights", &snapshot.bias_weights)?;
    push_i16(payload, snapshot.update_threshold);
    push_i16_vec(
        payload,
        "sc-per-pc-update-threshold",
        &snapshot.per_pc_update_threshold,
    )?;
    push_usize(payload, "sc-thread-count", snapshot.threads.len())?;
    for thread in &snapshot.threads {
        push_u64(payload, thread.global_history);
        push_u64(payload, thread.backward_history);
        push_u64(payload, thread.imli_count);
        push_u64(payload, thread.path_history);
        push_u64_vec(
            payload,
            "sc-first-local-histories",
            &thread.first_local_histories,
        )?;
    }
    push_i8(payload, snapshot.first_chooser);
    push_i8(payload, snapshot.second_chooser);
    push_u64(payload, snapshot.lookup_count);
    push_u64(payload, snapshot.update_count);
    push_u64(payload, snapshot.history_update_count);
    push_u64(payload, snapshot.correct_count);
    push_u64(payload, snapshot.wrong_count);
    Ok(())
}

fn decode_statistical_corrector_snapshot(
    payload: &[u8],
    offset: &mut usize,
    config: &StatisticalCorrectorConfig,
) -> Result<StatisticalCorrectorSnapshot, TageScLBranchPredictorError> {
    let global_gehl = read_expected_i8_matrix(
        "sc-global-gehl",
        payload,
        offset,
        config.global_lengths.len(),
        1usize << config.log_global,
    )?;
    check_i8_matrix("sc-global-gehl", &global_gehl, config.counter_width)?;
    let backward_gehl = read_expected_i8_matrix(
        "sc-backward-gehl",
        payload,
        offset,
        config.backward_lengths.len(),
        1usize << config.log_backward,
    )?;
    check_i8_matrix("sc-backward-gehl", &backward_gehl, config.counter_width)?;
    let local_gehl = read_expected_i8_matrix(
        "sc-local-gehl",
        payload,
        offset,
        config.local_lengths.len(),
        1usize << config.log_local,
    )?;
    check_i8_matrix("sc-local-gehl", &local_gehl, config.counter_width)?;
    let imli_gehl = read_expected_i8_matrix(
        "sc-imli-gehl",
        payload,
        offset,
        config.imli_lengths.len(),
        1usize << config.log_imli,
    )?;
    check_i8_matrix("sc-imli-gehl", &imli_gehl, config.counter_width)?;
    let global_weights = read_expected_i8_vec(
        "sc-global-weights",
        payload,
        offset,
        sc_update_weight_entries(config),
    )?;
    check_i8_vec(
        "sc-global-weights",
        &global_weights,
        config.extra_weights_width,
    )?;
    let backward_weights = read_expected_i8_vec(
        "sc-backward-weights",
        payload,
        offset,
        sc_update_weight_entries(config),
    )?;
    check_i8_vec(
        "sc-backward-weights",
        &backward_weights,
        config.extra_weights_width,
    )?;
    let local_weights = read_expected_i8_vec(
        "sc-local-weights",
        payload,
        offset,
        sc_update_weight_entries(config),
    )?;
    check_i8_vec(
        "sc-local-weights",
        &local_weights,
        config.extra_weights_width,
    )?;
    let imli_weights = read_expected_i8_vec(
        "sc-imli-weights",
        payload,
        offset,
        sc_update_weight_entries(config),
    )?;
    check_i8_vec("sc-imli-weights", &imli_weights, config.extra_weights_width)?;
    let bias = read_expected_i8_vec("sc-bias", payload, offset, sc_bias_entries(config))?;
    check_i8_vec("sc-bias", &bias, config.counter_width)?;
    let bias_sk = read_expected_i8_vec("sc-bias-sk", payload, offset, sc_bias_entries(config))?;
    check_i8_vec("sc-bias-sk", &bias_sk, config.counter_width)?;
    let bias_bank = read_expected_i8_vec("sc-bias-bank", payload, offset, sc_bias_entries(config))?;
    check_i8_vec("sc-bias-bank", &bias_bank, config.counter_width)?;
    let bias_weights = read_expected_i8_vec(
        "sc-bias-weights",
        payload,
        offset,
        sc_update_weight_entries(config),
    )?;
    check_i8_vec("sc-bias-weights", &bias_weights, config.extra_weights_width)?;
    let update_threshold = read_i16(payload, offset)?;
    check_signed_counter(
        "sc-update-threshold",
        i64::from(update_threshold),
        config.update_threshold_width,
    )?;
    let per_pc_update_threshold = read_expected_i16_vec(
        "sc-per-pc-update-threshold",
        payload,
        offset,
        sc_update_entries(config),
    )?;
    for threshold in &per_pc_update_threshold {
        check_signed_counter(
            "sc-per-pc-update-threshold",
            i64::from(*threshold),
            config.per_pc_threshold_width,
        )?;
    }
    let thread_count = read_usize(payload, offset)?;
    expect_len("sc-thread-count", config.threads, thread_count)?;
    let mut threads = Vec::with_capacity(thread_count);
    for _ in 0..thread_count {
        let global_history = read_u64(payload, offset)?;
        let backward_history = read_u64(payload, offset)?;
        let imli_count = read_u64(payload, offset)?;
        let path_history = read_u64(payload, offset)?;
        let first_local_histories = read_expected_u64_vec(
            "sc-first-local-histories",
            payload,
            offset,
            config.num_entries_first_local_histories,
        )?;
        threads.push(StatisticalCorrectorThreadSnapshot::from_parts(
            global_history,
            backward_history,
            imli_count,
            path_history,
            first_local_histories,
        ));
    }
    let first_chooser = read_i8(payload, offset)?;
    let second_chooser = read_i8(payload, offset)?;
    check_signed_counter(
        "sc-first-chooser",
        i64::from(first_chooser),
        config.chooser_conf_width,
    )?;
    check_signed_counter(
        "sc-second-chooser",
        i64::from(second_chooser),
        config.chooser_conf_width,
    )?;
    let lookup_count = read_u64(payload, offset)?;
    let update_count = read_u64(payload, offset)?;
    let history_update_count = read_u64(payload, offset)?;
    let correct_count = read_u64(payload, offset)?;
    let wrong_count = read_u64(payload, offset)?;

    Ok(StatisticalCorrectorSnapshot::from_parts(
        config.clone(),
        global_gehl,
        backward_gehl,
        local_gehl,
        imli_gehl,
        global_weights,
        backward_weights,
        local_weights,
        imli_weights,
        bias,
        bias_sk,
        bias_bank,
        bias_weights,
        update_threshold,
        per_pc_update_threshold,
        threads,
        first_chooser,
        second_chooser,
        lookup_count,
        update_count,
        history_update_count,
        correct_count,
        wrong_count,
    ))
}

fn push_i8_matrix(
    payload: &mut Vec<u8>,
    name: &'static str,
    matrix: &[Vec<i8>],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, matrix.len())?;
    for row in matrix {
        push_i8_vec(payload, name, row)?;
    }
    Ok(())
}

fn read_expected_i8_matrix(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected_rows: usize,
    expected_columns: usize,
) -> Result<Vec<Vec<i8>>, TageScLBranchPredictorError> {
    let rows = read_usize(payload, offset)?;
    expect_len(name, expected_rows, rows)?;
    let mut matrix = Vec::with_capacity(rows);
    for _ in 0..rows {
        matrix.push(read_expected_i8_vec(
            name,
            payload,
            offset,
            expected_columns,
        )?);
    }
    Ok(matrix)
}

fn check_i8_matrix(
    name: &'static str,
    matrix: &[Vec<i8>],
    bits: u8,
) -> Result<(), TageScLBranchPredictorError> {
    for row in matrix {
        check_i8_vec(name, row, bits)?;
    }
    Ok(())
}

fn check_i8_vec(
    name: &'static str,
    values: &[i8],
    bits: u8,
) -> Result<(), TageScLBranchPredictorError> {
    for value in values {
        check_signed_counter(name, i64::from(*value), bits)?;
    }
    Ok(())
}

fn sc_bias_entries(config: &StatisticalCorrectorConfig) -> usize {
    1usize << config.log_bias
}

fn sc_update_entries(config: &StatisticalCorrectorConfig) -> usize {
    1usize << config.log_size_up
}

fn sc_update_weight_entries(config: &StatisticalCorrectorConfig) -> usize {
    1usize << config.log_size_ups
}

fn push_usize(
    payload: &mut Vec<u8>,
    name: &'static str,
    value: usize,
) -> Result<(), TageScLBranchPredictorError> {
    push_u32(payload, require_u32(name, value)?);
    Ok(())
}

fn push_u8_vec(
    payload: &mut Vec<u8>,
    name: &'static str,
    values: &[u8],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, values.len())?;
    payload.extend_from_slice(values);
    Ok(())
}

fn push_i8_vec(
    payload: &mut Vec<u8>,
    name: &'static str,
    values: &[i8],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, values.len())?;
    for value in values {
        push_i8(payload, *value);
    }
    Ok(())
}

fn push_i16_vec(
    payload: &mut Vec<u8>,
    name: &'static str,
    values: &[i16],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, values.len())?;
    for value in values {
        push_i16(payload, *value);
    }
    Ok(())
}

fn push_u64_vec(
    payload: &mut Vec<u8>,
    name: &'static str,
    values: &[u64],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, values.len())?;
    for value in values {
        push_u64(payload, *value);
    }
    Ok(())
}

fn push_bool_vec(
    payload: &mut Vec<u8>,
    name: &'static str,
    values: &[bool],
) -> Result<(), TageScLBranchPredictorError> {
    push_usize(payload, name, values.len())?;
    for value in values {
        push_bool(payload, *value);
    }
    Ok(())
}

fn push_u8(payload: &mut Vec<u8>, value: u8) {
    payload.push(value);
}

fn push_i8(payload: &mut Vec<u8>, value: i8) {
    payload.push(value as u8);
}

fn push_bool(payload: &mut Vec<u8>, value: bool) {
    payload.push(u8::from(value));
}

fn push_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_i16(payload: &mut Vec<u8>, value: i16) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn push_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn read_bounded_u8_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    max: usize,
) -> Result<Vec<u8>, TageScLBranchPredictorError> {
    let len = read_usize(payload, offset)?;
    if len > max {
        return Err(TageScLBranchPredictorError::InvalidCheckpointVectorLength {
            name,
            expected: max,
            actual: len,
        });
    }
    let end = checked_offset(*offset, len)?;
    let bytes = payload.get(*offset..end).ok_or(
        TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.to_vec())
}

fn read_expected_u8_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected: usize,
) -> Result<Vec<u8>, TageScLBranchPredictorError> {
    let len = read_usize(payload, offset)?;
    expect_len(name, expected, len)?;
    let end = checked_offset(*offset, len)?;
    let bytes = payload.get(*offset..end).ok_or(
        TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.to_vec())
}

fn read_expected_i8_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected: usize,
) -> Result<Vec<i8>, TageScLBranchPredictorError> {
    Ok(read_expected_u8_vec(name, payload, offset, expected)?
        .into_iter()
        .map(|value| value as i8)
        .collect())
}

fn read_expected_i16_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected: usize,
) -> Result<Vec<i16>, TageScLBranchPredictorError> {
    let len = read_usize(payload, offset)?;
    expect_len(name, expected, len)?;
    require_remaining(payload, *offset, checked_product(name, len, 2)?)?;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(read_i16(payload, offset)?);
    }
    Ok(values)
}

fn read_expected_u64_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected: usize,
) -> Result<Vec<u64>, TageScLBranchPredictorError> {
    let len = read_usize(payload, offset)?;
    expect_len(name, expected, len)?;
    require_remaining(payload, *offset, checked_product(name, len, 8)?)?;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(read_u64(payload, offset)?);
    }
    Ok(values)
}

fn read_expected_bool_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    expected: usize,
) -> Result<Vec<bool>, TageScLBranchPredictorError> {
    let len = read_usize(payload, offset)?;
    expect_len(name, expected, len)?;
    require_remaining(payload, *offset, len)?;
    let mut values = Vec::with_capacity(len);
    for _ in 0..len {
        values.push(read_bool(name, payload, offset)?);
    }
    Ok(values)
}

fn read_usize(payload: &[u8], offset: &mut usize) -> Result<usize, TageScLBranchPredictorError> {
    Ok(read_u32(payload, offset)? as usize)
}

fn read_bool(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
) -> Result<bool, TageScLBranchPredictorError> {
    match read_u8(payload, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(TageScLBranchPredictorError::InvalidCheckpointBool { name, value }),
    }
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, TageScLBranchPredictorError> {
    let value =
        *payload
            .get(*offset)
            .ok_or(TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
                expected: *offset + 1,
                actual: payload.len(),
            })?;
    *offset += 1;
    Ok(value)
}

fn read_i8(payload: &[u8], offset: &mut usize) -> Result<i8, TageScLBranchPredictorError> {
    Ok(read_u8(payload, offset)? as i8)
}

fn read_u16(payload: &[u8], offset: &mut usize) -> Result<u16, TageScLBranchPredictorError> {
    Ok(u16::from_le_bytes(read_array(payload, offset)?))
}

fn read_i16(payload: &[u8], offset: &mut usize) -> Result<i16, TageScLBranchPredictorError> {
    Ok(i16::from_le_bytes(read_array(payload, offset)?))
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, TageScLBranchPredictorError> {
    Ok(u32::from_le_bytes(read_array(payload, offset)?))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, TageScLBranchPredictorError> {
    Ok(u64::from_le_bytes(read_array(payload, offset)?))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], TageScLBranchPredictorError> {
    let end = checked_offset(*offset, N)?;
    let bytes = payload.get(*offset..end).ok_or(
        TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.try_into().expect("slice length matches array length"))
}

fn checked_offset(base: usize, increment: usize) -> Result<usize, TageScLBranchPredictorError> {
    base.checked_add(increment)
        .ok_or(TageScLBranchPredictorError::CheckpointValueTooLarge {
            name: "payload-offset",
            value: increment,
            max: usize::MAX - base,
        })
}

fn checked_product(
    name: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TageScLBranchPredictorError> {
    left.checked_mul(right)
        .ok_or(TageScLBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: left,
            max: usize::MAX / right,
        })
}

fn checked_sum(
    name: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, TageScLBranchPredictorError> {
    left.checked_add(right)
        .ok_or(TageScLBranchPredictorError::CheckpointValueTooLarge {
            name,
            value: left,
            max: usize::MAX - right,
        })
}

fn require_remaining(
    payload: &[u8],
    offset: usize,
    needed: usize,
) -> Result<(), TageScLBranchPredictorError> {
    let end = checked_offset(offset, needed)?;
    if end <= payload.len() {
        Ok(())
    } else {
        Err(TageScLBranchPredictorError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        })
    }
}

fn require_u32(name: &'static str, value: usize) -> Result<u32, TageScLBranchPredictorError> {
    u32::try_from(value).map_err(|_| TageScLBranchPredictorError::CheckpointValueTooLarge {
        name,
        value,
        max: CHECKPOINT_U32_MAX,
    })
}

fn expect_len(
    name: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), TageScLBranchPredictorError> {
    if expected == actual {
        Ok(())
    } else {
        Err(TageScLBranchPredictorError::InvalidCheckpointVectorLength {
            name,
            expected,
            actual,
        })
    }
}

fn check_signed_counter(
    name: &'static str,
    value: i64,
    bits: u8,
) -> Result<(), TageScLBranchPredictorError> {
    let min = signed_min(bits);
    let max = signed_max(bits);
    if (min..=max).contains(&value) {
        Ok(())
    } else {
        Err(TageScLBranchPredictorError::InvalidCheckpointCounter {
            name,
            value,
            min,
            max,
        })
    }
}

fn check_unsigned_counter(
    name: &'static str,
    value: u64,
    bits: u8,
) -> Result<(), TageScLBranchPredictorError> {
    check_unsigned_value(name, value, bit_mask_u64(bits))
}

fn check_unsigned_value(
    name: &'static str,
    value: u64,
    max: u64,
) -> Result<(), TageScLBranchPredictorError> {
    if value <= max {
        Ok(())
    } else {
        Err(TageScLBranchPredictorError::InvalidCheckpointCounter {
            name,
            value: value as i64,
            min: 0,
            max: max as i64,
        })
    }
}

fn signed_min(bits: u8) -> i64 {
    -(1_i64 << (bits - 1))
}

fn signed_max(bits: u8) -> i64 {
    (1_i64 << (bits - 1)) - 1
}

fn bit_mask_u64(bits: u8) -> u64 {
    if bits >= u64::BITS as u8 {
        u64::MAX
    } else {
        (1_u64 << bits) - 1
    }
}
