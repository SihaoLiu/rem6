use crate::multiperspective_perceptron::{
    allocate_table_entries, max_global_history, max_path_entries, max_recency_entries,
    MultiperspectivePerceptronConfig, MultiperspectivePerceptronError,
    MultiperspectivePerceptronFeature, MultiperspectivePerceptronFeatureKind,
    MultiperspectivePerceptronFilterEntry, MultiperspectivePerceptronSnapshot,
    MultiperspectivePerceptronThreadSnapshot, MultiperspectivePerceptronWeight,
};
use crate::multiperspective_perceptron_snapshot::validate_snapshot_shape;

const CHECKPOINT_MAGIC: [u8; 4] = *b"RMPP";
const CHECKPOINT_VERSION: u8 = 1;
const U16_BYTES: usize = 2;
const I16_BYTES: usize = 2;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CHECKPOINT_U32_MAX: usize = u32::MAX as usize;
const CHECKPOINT_BOOL_BYTES: usize = 1;
const CHECKPOINT_WEIGHT_MAGNITUDE_BYTES: usize = 1;
const CHECKPOINT_FEATURE_BYTES: usize = 1 + I16_BYTES * 4 + U32_BYTES + 1;
const CHECKPOINT_HEADER_BYTES: usize = CHECKPOINT_MAGIC.len()
    + 1
    + U32_BYTES * 8
    + 5
    + 3
    + I16_BYTES * 8
    + U16_BYTES
    + U64_BYTES * 3
    + 3
    + I16_BYTES * 2
    + U64_BYTES * 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronCheckpointPayload {
    snapshot: MultiperspectivePerceptronSnapshot,
}

impl MultiperspectivePerceptronCheckpointPayload {
    pub fn from_snapshot(
        snapshot: MultiperspectivePerceptronSnapshot,
    ) -> Result<Self, MultiperspectivePerceptronError> {
        validate_snapshot(&snapshot)?;
        Ok(Self { snapshot })
    }

    pub const fn snapshot(&self) -> &MultiperspectivePerceptronSnapshot {
        &self.snapshot
    }

    pub fn into_snapshot(self) -> MultiperspectivePerceptronSnapshot {
        self.snapshot
    }

    pub fn encode(&self) -> Vec<u8> {
        let expected_len =
            checkpoint_payload_len(&self.snapshot.config, &self.snapshot.table_entries)
                .expect("validated multiperspective perceptron checkpoint length is representable");
        let mut payload = Vec::with_capacity(expected_len);

        payload.extend_from_slice(&CHECKPOINT_MAGIC);
        payload.push(CHECKPOINT_VERSION);
        encode_config_header(&mut payload, &self.snapshot.config);
        payload.extend_from_slice(&self.snapshot.theta.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.threshold_counter.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.lookup_count.to_le_bytes());
        payload.extend_from_slice(&self.snapshot.update_count.to_le_bytes());
        for feature in &self.snapshot.config.features {
            encode_feature(&mut payload, feature);
        }
        for table in &self.snapshot.tables {
            for weight in table {
                payload.push(weight.magnitude);
                for sign_bit in &weight.sign_bits {
                    encode_bool(&mut payload, *sign_bit);
                }
            }
        }
        for thread in &self.snapshot.threads {
            encode_thread(&mut payload, thread);
        }
        for mpred in &self.snapshot.mpreds {
            payload.extend_from_slice(&mpred.to_le_bytes());
        }

        debug_assert_eq!(payload.len(), expected_len);
        payload
    }

    pub fn decode(payload: &[u8]) -> Result<Self, MultiperspectivePerceptronError> {
        if payload.len() < CHECKPOINT_HEADER_BYTES {
            return Err(
                MultiperspectivePerceptronError::InvalidCheckpointPayloadSize {
                    expected: CHECKPOINT_HEADER_BYTES,
                    actual: payload.len(),
                },
            );
        }
        if payload[0..CHECKPOINT_MAGIC.len()] != CHECKPOINT_MAGIC {
            return Err(MultiperspectivePerceptronError::InvalidCheckpointMagic);
        }

        let mut offset = CHECKPOINT_MAGIC.len();
        let version = read_u8(payload, &mut offset)?;
        if version != CHECKPOINT_VERSION {
            return Err(MultiperspectivePerceptronError::UnsupportedCheckpointVersion { version });
        }

        let threads = read_u32(payload, &mut offset)? as usize;
        let num_filter_entries = read_u32(payload, &mut offset)? as usize;
        let num_local_histories = read_u32(payload, &mut offset)? as usize;
        let nbest = read_u32(payload, &mut offset)? as usize;
        let decay = read_u32(payload, &mut offset)? as usize;
        let budget_bits = read_u32(payload, &mut offset)? as usize;
        let initial_ghist_length = read_u32(payload, &mut offset)? as usize;
        let feature_count = read_u32(payload, &mut offset)? as usize;
        let local_history_length = read_u8(payload, &mut offset)?;
        let block_size = read_u8(payload, &mut offset)?;
        let tune_bits = read_u8(payload, &mut offset)?;
        let n_sign_bits = read_u8(payload, &mut offset)?;
        let pcbit = read_u8(payload, &mut offset)?;
        let pc_shift = read_i8(payload, &mut offset)?;
        let hshift = read_i8(payload, &mut offset)?;
        let extra_rounds = read_i8(payload, &mut offset)?;
        let threshold = read_i16(payload, &mut offset)?;
        let bias0 = read_i16(payload, &mut offset)?;
        let bias1 = read_i16(payload, &mut offset)?;
        let bias_mostly0 = read_i16(payload, &mut offset)?;
        let bias_mostly1 = read_i16(payload, &mut offset)?;
        let fudge_q6 = read_i16(payload, &mut offset)?;
        let speed = read_i16(payload, &mut offset)?;
        let initial_theta = read_i16(payload, &mut offset)?;
        let record_mask = read_u16(payload, &mut offset)?;
        let imli_mask1 = read_u64(payload, &mut offset)?;
        let imli_mask4 = read_u64(payload, &mut offset)?;
        let recencypos_mask = read_u64(payload, &mut offset)?;
        let hash_taken = read_bool("hash-taken", payload, &mut offset)?;
        let tune_only = read_bool("tune-only", payload, &mut offset)?;
        let ignore_path_size = read_bool("ignore-path-size", payload, &mut offset)?;
        let theta = read_i16(payload, &mut offset)?;
        let threshold_counter = read_i16(payload, &mut offset)?;
        let lookup_count = read_u64(payload, &mut offset)?;
        let update_count = read_u64(payload, &mut offset)?;

        let feature_bytes = checked_product("features", feature_count, CHECKPOINT_FEATURE_BYTES)?;
        let min_len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, feature_bytes)?;
        if payload.len() < min_len {
            return Err(
                MultiperspectivePerceptronError::InvalidCheckpointPayloadSize {
                    expected: min_len,
                    actual: payload.len(),
                },
            );
        }

        let mut features = Vec::with_capacity(feature_count);
        for _ in 0..feature_count {
            features.push(read_feature(payload, &mut offset)?);
        }

        let config = MultiperspectivePerceptronConfig::with_options(
            threads,
            num_filter_entries,
            num_local_histories,
            local_history_length,
            block_size,
            pc_shift,
            threshold,
            bias0,
            bias1,
            bias_mostly0,
            bias_mostly1,
            nbest,
            tune_bits,
            hshift,
            imli_mask1,
            imli_mask4,
            recencypos_mask,
            fudge_q6,
            n_sign_bits,
            pcbit,
            decay,
            record_mask,
            hash_taken,
            tune_only,
            extra_rounds,
            speed,
            initial_theta,
            budget_bits,
            initial_ghist_length,
            ignore_path_size,
            features,
        )?;
        let table_entries = allocate_table_entries(&config)?;
        let expected_len = checkpoint_payload_len(&config, &table_entries)?;
        if payload.len() != expected_len {
            return Err(
                MultiperspectivePerceptronError::InvalidCheckpointPayloadSize {
                    expected: expected_len,
                    actual: payload.len(),
                },
            );
        }

        let mut tables = Vec::with_capacity(config.features.len());
        for (feature_index, entries) in table_entries.iter().copied().enumerate() {
            let max_magnitude = max_magnitude(config.features[feature_index].width);
            let mut table = Vec::with_capacity(entries);
            for table_index in 0..entries {
                let magnitude = read_u8(payload, &mut offset)?;
                let sign_bits = read_bool_vec(
                    "weight-sign-bit",
                    payload,
                    &mut offset,
                    n_sign_bits as usize,
                )?;
                validate_weight(
                    feature_index,
                    table_index,
                    magnitude,
                    sign_bits.len(),
                    max_magnitude,
                    n_sign_bits as usize,
                )?;
                table.push(MultiperspectivePerceptronWeight {
                    magnitude,
                    sign_bits,
                });
            }
            tables.push(table);
        }

        let max_global_history = max_global_history(&config);
        let max_path_entries = max_path_entries(&config).max(1);
        let max_recency_entries = max_recency_entries(&config);
        let mut thread_snapshots = Vec::with_capacity(config.threads);
        for _ in 0..config.threads {
            thread_snapshots.push(read_thread(
                payload,
                &mut offset,
                &config,
                max_global_history,
                max_path_entries,
                max_recency_entries,
            )?);
        }
        let mut mpreds = Vec::with_capacity(config.features.len());
        for _ in 0..config.features.len() {
            mpreds.push(read_u64(payload, &mut offset)?);
        }
        debug_assert_eq!(offset, payload.len());

        Self::from_snapshot(MultiperspectivePerceptronSnapshot {
            config,
            table_entries,
            tables,
            threads: thread_snapshots,
            mpreds,
            theta,
            threshold_counter,
            lookup_count,
            update_count,
        })
    }
}

fn encode_config_header(payload: &mut Vec<u8>, config: &MultiperspectivePerceptronConfig) {
    payload.extend_from_slice(&(config.threads as u32).to_le_bytes());
    payload.extend_from_slice(&(config.num_filter_entries as u32).to_le_bytes());
    payload.extend_from_slice(&(config.num_local_histories as u32).to_le_bytes());
    payload.extend_from_slice(&(config.nbest as u32).to_le_bytes());
    payload.extend_from_slice(&(config.decay as u32).to_le_bytes());
    payload.extend_from_slice(&(config.budget_bits as u32).to_le_bytes());
    payload.extend_from_slice(&(config.initial_ghist_length as u32).to_le_bytes());
    payload.extend_from_slice(&(config.features.len() as u32).to_le_bytes());
    payload.push(config.local_history_length);
    payload.push(config.block_size);
    payload.push(config.tune_bits);
    payload.push(config.n_sign_bits);
    payload.push(config.pcbit);
    payload.extend_from_slice(&config.pc_shift.to_le_bytes());
    payload.extend_from_slice(&config.hshift.to_le_bytes());
    payload.extend_from_slice(&config.extra_rounds.to_le_bytes());
    payload.extend_from_slice(&config.threshold.to_le_bytes());
    payload.extend_from_slice(&config.bias0.to_le_bytes());
    payload.extend_from_slice(&config.bias1.to_le_bytes());
    payload.extend_from_slice(&config.bias_mostly0.to_le_bytes());
    payload.extend_from_slice(&config.bias_mostly1.to_le_bytes());
    payload.extend_from_slice(&config.fudge_q6.to_le_bytes());
    payload.extend_from_slice(&config.speed.to_le_bytes());
    payload.extend_from_slice(&config.initial_theta.to_le_bytes());
    payload.extend_from_slice(&config.record_mask.to_le_bytes());
    payload.extend_from_slice(&config.imli_mask1.to_le_bytes());
    payload.extend_from_slice(&config.imli_mask4.to_le_bytes());
    payload.extend_from_slice(&config.recencypos_mask.to_le_bytes());
    encode_bool(payload, config.hash_taken);
    encode_bool(payload, config.tune_only);
    encode_bool(payload, config.ignore_path_size);
}

fn encode_feature(payload: &mut Vec<u8>, feature: &MultiperspectivePerceptronFeature) {
    payload.push(encode_feature_kind(feature.kind));
    payload.extend_from_slice(&feature.p1.to_le_bytes());
    payload.extend_from_slice(&feature.p2.to_le_bytes());
    payload.extend_from_slice(&feature.p3.to_le_bytes());
    payload.extend_from_slice(&feature.coefficient_q6.to_le_bytes());
    payload.extend_from_slice(&(feature.table_entries as u32).to_le_bytes());
    payload.push(feature.width);
}

fn encode_thread(payload: &mut Vec<u8>, thread: &MultiperspectivePerceptronThreadSnapshot) {
    for entry in &thread.filter_table {
        encode_bool(payload, entry.seen_taken);
        encode_bool(payload, entry.seen_untaken);
    }
    for bit in &thread.global_history {
        encode_bool(payload, *bit);
    }
    for history in &thread.local_histories {
        payload.extend_from_slice(&history.to_le_bytes());
    }
    for path in &thread.path_history {
        payload.extend_from_slice(&path.to_le_bytes());
    }
    for recency in &thread.recency_stack {
        payload.extend_from_slice(&recency.to_le_bytes());
    }
    for counter in &thread.imli_counters {
        payload.extend_from_slice(&counter.to_le_bytes());
    }
    encode_bool(payload, thread.last_ghist_bit);
}

fn read_feature(
    payload: &[u8],
    offset: &mut usize,
) -> Result<MultiperspectivePerceptronFeature, MultiperspectivePerceptronError> {
    let kind = decode_feature_kind(read_u8(payload, offset)?)?;
    let p1 = read_i16(payload, offset)?;
    let p2 = read_i16(payload, offset)?;
    let p3 = read_i16(payload, offset)?;
    let coefficient_q6 = read_i16(payload, offset)?;
    let table_entries = read_u32(payload, offset)? as usize;
    let width = read_u8(payload, offset)?;
    Ok(MultiperspectivePerceptronFeature::new(
        kind,
        p1,
        p2,
        p3,
        coefficient_q6,
        table_entries,
        width,
    ))
}

fn read_thread(
    payload: &[u8],
    offset: &mut usize,
    config: &MultiperspectivePerceptronConfig,
    max_global_history: usize,
    max_path_entries: usize,
    max_recency_entries: usize,
) -> Result<MultiperspectivePerceptronThreadSnapshot, MultiperspectivePerceptronError> {
    let mut filter_table = Vec::with_capacity(config.num_filter_entries);
    for _ in 0..config.num_filter_entries {
        filter_table.push(MultiperspectivePerceptronFilterEntry {
            seen_taken: read_bool("filter-seen-taken", payload, offset)?,
            seen_untaken: read_bool("filter-seen-untaken", payload, offset)?,
        });
    }
    let global_history = read_bool_vec("global-history", payload, offset, max_global_history)?;
    let mut local_histories = Vec::with_capacity(config.num_local_histories);
    let local_history_mask = (1u64 << config.local_history_length) - 1;
    for _ in 0..config.num_local_histories {
        let history = read_u64(payload, offset)?;
        if history > local_history_mask {
            return Err(MultiperspectivePerceptronError::CheckpointValueTooLarge {
                name: "local-history",
                value: history as usize,
                max: local_history_mask as usize,
            });
        }
        local_histories.push(history);
    }
    let mut path_history = Vec::with_capacity(max_path_entries);
    for _ in 0..max_path_entries {
        path_history.push(read_u16(payload, offset)?);
    }
    let mut recency_stack = Vec::with_capacity(max_recency_entries);
    for _ in 0..max_recency_entries {
        recency_stack.push(read_u16(payload, offset)?);
    }
    let mut imli_counters = [0; 4];
    for counter in &mut imli_counters {
        *counter = read_u16(payload, offset)?;
    }
    let last_ghist_bit = read_bool("last-ghist-bit", payload, offset)?;

    Ok(MultiperspectivePerceptronThreadSnapshot {
        max_global_history,
        max_path_entries,
        filter_table,
        global_history,
        local_histories,
        path_history,
        recency_stack,
        imli_counters,
        last_ghist_bit,
    })
}

fn validate_snapshot(
    snapshot: &MultiperspectivePerceptronSnapshot,
) -> Result<(), MultiperspectivePerceptronError> {
    let config = &snapshot.config;
    require_u32("threads", config.threads)?;
    require_u32("filter-entries", config.num_filter_entries)?;
    require_u32("local-histories", config.num_local_histories)?;
    require_u32("nbest", config.nbest)?;
    require_u32("decay", config.decay)?;
    require_u32("budget-bits", config.budget_bits)?;
    require_u32("initial-ghist-length", config.initial_ghist_length)?;
    require_u32("features", config.features.len())?;
    for feature in &config.features {
        require_u32("feature-table-entries", feature.table_entries)?;
    }

    let expected_table_entries = allocate_table_entries(config)?;
    checkpoint_payload_len(config, &expected_table_entries)?;
    validate_snapshot_shape(config, snapshot)
}

fn validate_weight(
    feature_index: usize,
    table_index: usize,
    magnitude: u8,
    sign_bits: usize,
    max_magnitude: u8,
    expected_sign_bits: usize,
) -> Result<(), MultiperspectivePerceptronError> {
    if magnitude > max_magnitude || sign_bits != expected_sign_bits {
        return Err(MultiperspectivePerceptronError::InvalidCheckpointWeight {
            feature_index,
            table_index,
            magnitude,
            max_magnitude,
            sign_bits,
            expected_sign_bits,
        });
    }
    Ok(())
}

fn checkpoint_payload_len(
    config: &MultiperspectivePerceptronConfig,
    table_entries: &[usize],
) -> Result<usize, MultiperspectivePerceptronError> {
    let feature_bytes =
        checked_product("features", config.features.len(), CHECKPOINT_FEATURE_BYTES)?;
    let mut table_bytes = 0usize;
    let weight_bytes = checked_sum(
        "weight-bytes",
        CHECKPOINT_WEIGHT_MAGNITUDE_BYTES,
        config.n_sign_bits as usize,
    )?;
    for entries in table_entries {
        let bytes = checked_product("table-entries", *entries, weight_bytes)?;
        table_bytes = checked_sum("table-bytes", table_bytes, bytes)?;
    }
    let thread_bytes =
        checked_product("threads", config.threads, checkpoint_thread_bytes(config)?)?;
    let mpred_bytes = checked_product("mpreds", config.features.len(), U64_BYTES)?;

    let len = checked_sum("payload-size", CHECKPOINT_HEADER_BYTES, feature_bytes)?;
    let len = checked_sum("payload-size", len, table_bytes)?;
    let len = checked_sum("payload-size", len, thread_bytes)?;
    checked_sum("payload-size", len, mpred_bytes)
}

fn checkpoint_thread_bytes(
    config: &MultiperspectivePerceptronConfig,
) -> Result<usize, MultiperspectivePerceptronError> {
    let filter_bytes = checked_product(
        "filter-entries",
        config.num_filter_entries,
        CHECKPOINT_BOOL_BYTES * 2,
    )?;
    let local_history_bytes =
        checked_product("local-histories", config.num_local_histories, U64_BYTES)?;
    let path_bytes = checked_product("path-history", max_path_entries(config).max(1), U16_BYTES)?;
    let recency_bytes = checked_product("recency-stack", max_recency_entries(config), U16_BYTES)?;

    let len = checked_sum("thread-bytes", filter_bytes, max_global_history(config))?;
    let len = checked_sum("thread-bytes", len, local_history_bytes)?;
    let len = checked_sum("thread-bytes", len, path_bytes)?;
    let len = checked_sum("thread-bytes", len, recency_bytes)?;
    let len = checked_sum("thread-bytes", len, U16_BYTES * 4)?;
    checked_sum("thread-bytes", len, CHECKPOINT_BOOL_BYTES)
}

fn checked_product(
    name: &'static str,
    count: usize,
    bytes: usize,
) -> Result<usize, MultiperspectivePerceptronError> {
    count
        .checked_mul(bytes)
        .ok_or(MultiperspectivePerceptronError::CheckpointValueTooLarge {
            name,
            value: count,
            max: usize::MAX / bytes,
        })
}

fn checked_sum(
    name: &'static str,
    base: usize,
    increment: usize,
) -> Result<usize, MultiperspectivePerceptronError> {
    base.checked_add(increment)
        .ok_or(MultiperspectivePerceptronError::CheckpointValueTooLarge {
            name,
            value: increment,
            max: usize::MAX - base,
        })
}

fn checked_offset(base: usize, increment: usize) -> Result<usize, MultiperspectivePerceptronError> {
    checked_sum("payload-offset", base, increment)
}

fn require_u32(name: &'static str, value: usize) -> Result<u32, MultiperspectivePerceptronError> {
    u32::try_from(value).map_err(
        |_| MultiperspectivePerceptronError::CheckpointValueTooLarge {
            name,
            value,
            max: CHECKPOINT_U32_MAX,
        },
    )
}

fn encode_bool(payload: &mut Vec<u8>, value: bool) {
    payload.push(u8::from(value));
}

fn read_bool(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
) -> Result<bool, MultiperspectivePerceptronError> {
    match read_u8(payload, offset)? {
        0 => Ok(false),
        1 => Ok(true),
        value => Err(MultiperspectivePerceptronError::InvalidCheckpointBool { name, value }),
    }
}

fn read_bool_vec(
    name: &'static str,
    payload: &[u8],
    offset: &mut usize,
    count: usize,
) -> Result<Vec<bool>, MultiperspectivePerceptronError> {
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push(read_bool(name, payload, offset)?);
    }
    Ok(values)
}

fn encode_feature_kind(kind: MultiperspectivePerceptronFeatureKind) -> u8 {
    match kind {
        MultiperspectivePerceptronFeatureKind::Bias => 0,
        MultiperspectivePerceptronFeatureKind::GlobalHistory => 1,
        MultiperspectivePerceptronFeatureKind::GlobalHistoryPath => 2,
        MultiperspectivePerceptronFeatureKind::GlobalHistoryModuloPath => 3,
        MultiperspectivePerceptronFeatureKind::Imli => 4,
        MultiperspectivePerceptronFeatureKind::Local => 5,
        MultiperspectivePerceptronFeatureKind::Recency => 6,
        MultiperspectivePerceptronFeatureKind::RecencyPosition => 7,
        MultiperspectivePerceptronFeatureKind::ShiftedGlobalHistoryPath => 8,
        MultiperspectivePerceptronFeatureKind::Acyclic => 9,
        MultiperspectivePerceptronFeatureKind::BlurryPath => 10,
        MultiperspectivePerceptronFeatureKind::ModuloHistory => 11,
        MultiperspectivePerceptronFeatureKind::ModuloPath => 12,
        MultiperspectivePerceptronFeatureKind::Path => 13,
    }
}

fn decode_feature_kind(
    value: u8,
) -> Result<MultiperspectivePerceptronFeatureKind, MultiperspectivePerceptronError> {
    match value {
        0 => Ok(MultiperspectivePerceptronFeatureKind::Bias),
        1 => Ok(MultiperspectivePerceptronFeatureKind::GlobalHistory),
        2 => Ok(MultiperspectivePerceptronFeatureKind::GlobalHistoryPath),
        3 => Ok(MultiperspectivePerceptronFeatureKind::GlobalHistoryModuloPath),
        4 => Ok(MultiperspectivePerceptronFeatureKind::Imli),
        5 => Ok(MultiperspectivePerceptronFeatureKind::Local),
        6 => Ok(MultiperspectivePerceptronFeatureKind::Recency),
        7 => Ok(MultiperspectivePerceptronFeatureKind::RecencyPosition),
        8 => Ok(MultiperspectivePerceptronFeatureKind::ShiftedGlobalHistoryPath),
        9 => Ok(MultiperspectivePerceptronFeatureKind::Acyclic),
        10 => Ok(MultiperspectivePerceptronFeatureKind::BlurryPath),
        11 => Ok(MultiperspectivePerceptronFeatureKind::ModuloHistory),
        12 => Ok(MultiperspectivePerceptronFeatureKind::ModuloPath),
        13 => Ok(MultiperspectivePerceptronFeatureKind::Path),
        value => Err(MultiperspectivePerceptronError::InvalidCheckpointFeatureKind { value }),
    }
}

fn max_magnitude(width: u8) -> u8 {
    (1u8 << (width - 1)) - 1
}

fn read_u8(payload: &[u8], offset: &mut usize) -> Result<u8, MultiperspectivePerceptronError> {
    let value = *payload.get(*offset).ok_or(
        MultiperspectivePerceptronError::InvalidCheckpointPayloadSize {
            expected: *offset + 1,
            actual: payload.len(),
        },
    )?;
    *offset += 1;
    Ok(value)
}

fn read_i8(payload: &[u8], offset: &mut usize) -> Result<i8, MultiperspectivePerceptronError> {
    Ok(i8::from_le_bytes(read_array(payload, offset)?))
}

fn read_u16(payload: &[u8], offset: &mut usize) -> Result<u16, MultiperspectivePerceptronError> {
    Ok(u16::from_le_bytes(read_array(payload, offset)?))
}

fn read_i16(payload: &[u8], offset: &mut usize) -> Result<i16, MultiperspectivePerceptronError> {
    Ok(i16::from_le_bytes(read_array(payload, offset)?))
}

fn read_u32(payload: &[u8], offset: &mut usize) -> Result<u32, MultiperspectivePerceptronError> {
    Ok(u32::from_le_bytes(read_array(payload, offset)?))
}

fn read_u64(payload: &[u8], offset: &mut usize) -> Result<u64, MultiperspectivePerceptronError> {
    Ok(u64::from_le_bytes(read_array(payload, offset)?))
}

fn read_array<const N: usize>(
    payload: &[u8],
    offset: &mut usize,
) -> Result<[u8; N], MultiperspectivePerceptronError> {
    let end = checked_offset(*offset, N)?;
    let bytes = payload.get(*offset..end).ok_or(
        MultiperspectivePerceptronError::InvalidCheckpointPayloadSize {
            expected: end,
            actual: payload.len(),
        },
    )?;
    *offset = end;
    Ok(bytes.try_into().expect("slice length matches array length"))
}
