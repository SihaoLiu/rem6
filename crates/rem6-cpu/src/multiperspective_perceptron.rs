use std::cmp::Reverse;
use std::error::Error;
use std::fmt;

use rem6_memory::Address;

use crate::multiperspective_perceptron_snapshot::validate_snapshot_shape;
use crate::CpuId;

mod speculation;

const XLAT_6: [i16; 32] = [
    1, 3, 4, 5, 7, 8, 9, 11, 12, 14, 15, 17, 19, 21, 23, 25, 27, 29, 32, 34, 37, 41, 45, 49, 53,
    58, 63, 69, 76, 85, 94, 106,
];
const XLAT_5: [i16; 16] = [0, 4, 5, 7, 9, 11, 12, 14, 16, 17, 19, 22, 28, 33, 39, 45];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiperspectivePerceptronError {
    ZeroThreads,
    ZeroLocalHistories,
    LocalHistoryLengthOutOfRange {
        bits: u8,
    },
    BlockSizeOutOfRange {
        bits: u8,
    },
    EmptyFeatures,
    FeatureWidthOutOfRange {
        width: u8,
    },
    SignBitsOutOfRange {
        bits: u8,
    },
    BudgetTooSmall {
        budget_bits: usize,
    },
    UnknownThread {
        cpu: CpuId,
    },
    SnapshotConfigMismatch {
        expected: Box<MultiperspectivePerceptronConfig>,
        actual: Box<MultiperspectivePerceptronConfig>,
    },
    SnapshotShapeMismatch {
        expected_tables: usize,
        actual_tables: usize,
        expected_threads: usize,
        actual_threads: usize,
    },
    InvalidCheckpointPayloadSize {
        expected: usize,
        actual: usize,
    },
    SnapshotTableEntriesMismatch {
        feature_index: usize,
        expected: usize,
        actual: usize,
    },
    InvalidCheckpointMagic,
    UnsupportedCheckpointVersion {
        version: u8,
    },
    CheckpointValueTooLarge {
        name: &'static str,
        value: usize,
        max: usize,
    },
    InvalidCheckpointBool {
        name: &'static str,
        value: u8,
    },
    InvalidCheckpointFeatureKind {
        value: u8,
    },
    InvalidCheckpointWeight {
        feature_index: usize,
        table_index: usize,
        magnitude: u8,
        max_magnitude: u8,
        sign_bits: usize,
        expected_sign_bits: usize,
    },
}

impl fmt::Display for MultiperspectivePerceptronError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroThreads => write!(formatter, "multiperspective perceptron has no threads"),
            Self::ZeroLocalHistories => {
                write!(formatter, "multiperspective perceptron has no local histories")
            }
            Self::LocalHistoryLengthOutOfRange { bits } => write!(
                formatter,
                "multiperspective perceptron local history length {bits} is outside 1..=63"
            ),
            Self::BlockSizeOutOfRange { bits } => write!(
                formatter,
                "multiperspective perceptron block size {bits} is outside 1..=31"
            ),
            Self::EmptyFeatures => write!(formatter, "multiperspective perceptron has no features"),
            Self::FeatureWidthOutOfRange { width } => write!(
                formatter,
                "multiperspective perceptron feature width {width} is outside 2..=6"
            ),
            Self::SignBitsOutOfRange { bits } => write!(
                formatter,
                "multiperspective perceptron sign bits {bits} is outside 1..=8"
            ),
            Self::BudgetTooSmall { budget_bits } => write!(
                formatter,
                "multiperspective perceptron budget {budget_bits} bits cannot allocate all feature tables"
            ),
            Self::UnknownThread { cpu } => write!(
                formatter,
                "multiperspective perceptron thread {} is not configured",
                cpu.get()
            ),
            Self::SnapshotConfigMismatch { expected, actual } => write!(
                formatter,
                "multiperspective perceptron snapshot config {actual:?} does not match predictor config {expected:?}"
            ),
            Self::SnapshotShapeMismatch {
                expected_tables,
                actual_tables,
                expected_threads,
                actual_threads,
            } => write!(
                formatter,
                "multiperspective perceptron snapshot shape tables={actual_tables}, threads={actual_threads} does not match predictor tables={expected_tables}, threads={expected_threads}"
            ),
            Self::InvalidCheckpointPayloadSize { expected, actual } => write!(
                formatter,
                "multiperspective perceptron checkpoint payload has {actual} bytes; expected {expected}"
            ),
            Self::SnapshotTableEntriesMismatch {
                feature_index,
                expected,
                actual,
            } => write!(
                formatter,
                "multiperspective perceptron snapshot feature {feature_index} has {actual} table entries; expected {expected}"
            ),
            Self::InvalidCheckpointMagic => write!(
                formatter,
                "multiperspective perceptron checkpoint payload has invalid magic"
            ),
            Self::UnsupportedCheckpointVersion { version } => write!(
                formatter,
                "multiperspective perceptron checkpoint payload version {version} is not supported"
            ),
            Self::CheckpointValueTooLarge { name, value, max } => write!(
                formatter,
                "multiperspective perceptron checkpoint {name} value {value} exceeds maximum {max}"
            ),
            Self::InvalidCheckpointBool { name, value } => write!(
                formatter,
                "multiperspective perceptron checkpoint {name} bool has invalid value {value}"
            ),
            Self::InvalidCheckpointFeatureKind { value } => write!(
                formatter,
                "multiperspective perceptron checkpoint feature kind {value} is invalid"
            ),
            Self::InvalidCheckpointWeight {
                feature_index,
                table_index,
                magnitude,
                max_magnitude,
                sign_bits,
                expected_sign_bits,
            } => write!(
                formatter,
                "multiperspective perceptron checkpoint weight feature={feature_index}, table={table_index} has magnitude {magnitude}/{max_magnitude} and {sign_bits}/{expected_sign_bits} sign bits"
            ),
        }
    }
}

impl Error for MultiperspectivePerceptronError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiperspectivePerceptronFeatureKind {
    Bias,
    GlobalHistory,
    GlobalHistoryPath,
    GlobalHistoryModuloPath,
    Imli,
    Local,
    Recency,
    RecencyPosition,
    ShiftedGlobalHistoryPath,
    Acyclic,
    BlurryPath,
    ModuloHistory,
    ModuloPath,
    Path,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronFeature {
    pub(crate) kind: MultiperspectivePerceptronFeatureKind,
    pub(crate) p1: i16,
    pub(crate) p2: i16,
    pub(crate) p3: i16,
    pub(crate) coefficient_q6: i16,
    pub(crate) table_entries: usize,
    pub(crate) width: u8,
}

impl MultiperspectivePerceptronFeature {
    pub const fn bias(coefficient_q6: i16, table_entries: usize, width: u8) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::Bias,
            0,
            0,
            0,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn global_history(
        start: i16,
        end: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::GlobalHistory,
            start,
            end,
            0,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn global_history_path(
        history_start: i16,
        path_len: i16,
        style: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::GlobalHistoryPath,
            history_start,
            path_len,
            style,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn global_history_modulo_path(
        modulo: i16,
        path_len: i16,
        shift: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::GlobalHistoryModuloPath,
            modulo,
            path_len,
            shift,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn imli(counter: i16, coefficient_q6: i16, table_entries: usize, width: u8) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::Imli,
            counter,
            0,
            0,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn local(coefficient_q6: i16, table_entries: usize, width: u8) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::Local,
            -1,
            0,
            0,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn recency(
        depth: i16,
        shift: i16,
        style: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::Recency,
            depth,
            shift,
            style,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn recency_position(
        depth: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::RecencyPosition,
            depth,
            0,
            0,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub const fn shifted_global_history_path(
        shift: i16,
        path_len: i16,
        path_shift: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self::new(
            MultiperspectivePerceptronFeatureKind::ShiftedGlobalHistoryPath,
            shift,
            path_len,
            path_shift,
            coefficient_q6,
            table_entries,
            width,
        )
    }

    pub(crate) const fn new(
        kind: MultiperspectivePerceptronFeatureKind,
        p1: i16,
        p2: i16,
        p3: i16,
        coefficient_q6: i16,
        table_entries: usize,
        width: u8,
    ) -> Self {
        Self {
            kind,
            p1,
            p2,
            p3,
            coefficient_q6,
            table_entries,
            width,
        }
    }

    pub const fn kind(&self) -> MultiperspectivePerceptronFeatureKind {
        self.kind
    }

    pub const fn coefficient_q6(&self) -> i16 {
        self.coefficient_q6
    }

    pub const fn table_entries(&self) -> usize {
        self.table_entries
    }

    pub const fn width(&self) -> u8 {
        self.width
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronConfig {
    pub(crate) threads: usize,
    pub(crate) num_filter_entries: usize,
    pub(crate) num_local_histories: usize,
    pub(crate) local_history_length: u8,
    pub(crate) block_size: u8,
    pub(crate) pc_shift: i8,
    pub(crate) threshold: i16,
    pub(crate) bias0: i16,
    pub(crate) bias1: i16,
    pub(crate) bias_mostly0: i16,
    pub(crate) bias_mostly1: i16,
    pub(crate) nbest: usize,
    pub(crate) tune_bits: u8,
    pub(crate) hshift: i8,
    pub(crate) imli_mask1: u64,
    pub(crate) imli_mask4: u64,
    pub(crate) recencypos_mask: u64,
    pub(crate) fudge_q6: i16,
    pub(crate) n_sign_bits: u8,
    pub(crate) pcbit: u8,
    pub(crate) decay: usize,
    pub(crate) record_mask: u16,
    pub(crate) hash_taken: bool,
    pub(crate) tune_only: bool,
    pub(crate) extra_rounds: i8,
    pub(crate) speed: i16,
    pub(crate) initial_theta: i16,
    pub(crate) budget_bits: usize,
    pub(crate) initial_ghist_length: usize,
    pub(crate) ignore_path_size: bool,
    pub(crate) features: Vec<MultiperspectivePerceptronFeature>,
}

impl MultiperspectivePerceptronConfig {
    pub fn eight_kb(threads: usize) -> Result<Self, MultiperspectivePerceptronError> {
        Self::with_options(
            threads,
            0,
            48,
            11,
            21,
            -10,
            1,
            -5,
            5,
            -1,
            1,
            20,
            24,
            -6,
            0x6,
            0x4400,
            0x100000090,
            16,
            2,
            2,
            0,
            191,
            false,
            true,
            1,
            9,
            10,
            8192 * 8 + 2048,
            1,
            false,
            vec![
                MultiperspectivePerceptronFeature::bias(154, 0, 6),
                MultiperspectivePerceptronFeature::global_history(0, 19, 92, 0, 6),
                MultiperspectivePerceptronFeature::global_history(0, 65, 64, 0, 6),
                MultiperspectivePerceptronFeature::global_history(21, 64, 64, 0, 6),
                MultiperspectivePerceptronFeature::global_history(75, 150, 68, 0, 6),
                MultiperspectivePerceptronFeature::global_history_modulo_path(0, 7, 3, 104, 0, 6),
                MultiperspectivePerceptronFeature::global_history_path(11, 2, -1, 80, 0, 6),
                MultiperspectivePerceptronFeature::global_history_path(15, 4, -1, 72, 0, 6),
                MultiperspectivePerceptronFeature::global_history_path(31, 1, -1, 90, 0, 6),
                MultiperspectivePerceptronFeature::global_history_path(7, 1, -1, 96, 600, 6),
                MultiperspectivePerceptronFeature::imli(4, 82, 375, 6),
                MultiperspectivePerceptronFeature::local(100, 512, 6),
                MultiperspectivePerceptronFeature::recency(14, 4, -1, 80, 0, 6),
                MultiperspectivePerceptronFeature::recency_position(31, 120, 0, 6),
                MultiperspectivePerceptronFeature::shifted_global_history_path(0, 4, 3, 106, 0, 6),
                MultiperspectivePerceptronFeature::shifted_global_history_path(1, 2, 5, 162, 0, 5),
            ],
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn with_options(
        threads: usize,
        num_filter_entries: usize,
        num_local_histories: usize,
        local_history_length: u8,
        block_size: u8,
        pc_shift: i8,
        threshold: i16,
        bias0: i16,
        bias1: i16,
        bias_mostly0: i16,
        bias_mostly1: i16,
        nbest: usize,
        tune_bits: u8,
        hshift: i8,
        imli_mask1: u64,
        imli_mask4: u64,
        recencypos_mask: u64,
        fudge_q6: i16,
        n_sign_bits: u8,
        pcbit: u8,
        decay: usize,
        record_mask: u16,
        hash_taken: bool,
        tune_only: bool,
        extra_rounds: i8,
        speed: i16,
        initial_theta: i16,
        budget_bits: usize,
        initial_ghist_length: usize,
        ignore_path_size: bool,
        features: Vec<MultiperspectivePerceptronFeature>,
    ) -> Result<Self, MultiperspectivePerceptronError> {
        if threads == 0 {
            return Err(MultiperspectivePerceptronError::ZeroThreads);
        }
        if num_local_histories == 0 {
            return Err(MultiperspectivePerceptronError::ZeroLocalHistories);
        }
        if !(1..=63).contains(&local_history_length) {
            return Err(
                MultiperspectivePerceptronError::LocalHistoryLengthOutOfRange {
                    bits: local_history_length,
                },
            );
        }
        if !(1..=31).contains(&block_size) {
            return Err(MultiperspectivePerceptronError::BlockSizeOutOfRange { bits: block_size });
        }
        if features.is_empty() {
            return Err(MultiperspectivePerceptronError::EmptyFeatures);
        }
        if !(1..=8).contains(&n_sign_bits) {
            return Err(MultiperspectivePerceptronError::SignBitsOutOfRange { bits: n_sign_bits });
        }
        for feature in &features {
            if !(2..=6).contains(&feature.width) {
                return Err(MultiperspectivePerceptronError::FeatureWidthOutOfRange {
                    width: feature.width,
                });
            }
        }

        Ok(Self {
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
        })
    }

    pub const fn threads(&self) -> usize {
        self.threads
    }

    pub const fn budget_bits(&self) -> usize {
        self.budget_bits
    }

    pub const fn num_local_histories(&self) -> usize {
        self.num_local_histories
    }

    pub const fn num_filter_entries(&self) -> usize {
        self.num_filter_entries
    }

    pub const fn local_history_length(&self) -> u8 {
        self.local_history_length
    }

    pub const fn pc_shift(&self) -> i8 {
        self.pc_shift
    }

    pub const fn threshold(&self) -> i16 {
        self.threshold
    }

    pub fn features(&self) -> &[MultiperspectivePerceptronFeature] {
        &self.features
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptron {
    config: MultiperspectivePerceptronConfig,
    table_entries: Vec<usize>,
    tables: Vec<Vec<MultiperspectivePerceptronWeight>>,
    threads: Vec<MultiperspectivePerceptronThreadSnapshot>,
    mpreds: Vec<u64>,
    theta: i16,
    threshold_counter: i16,
    lookup_count: u64,
    update_count: u64,
}

impl MultiperspectivePerceptron {
    pub fn new(
        config: MultiperspectivePerceptronConfig,
    ) -> Result<Self, MultiperspectivePerceptronError> {
        let table_entries = allocate_table_entries(&config)?;
        let max_global_history = max_global_history(&config);
        let path_entries = max_path_entries(&config);
        let recency_entries = max_recency_entries(&config);
        let tables = table_entries
            .iter()
            .map(|entries| {
                vec![MultiperspectivePerceptronWeight::new(config.n_sign_bits); *entries]
            })
            .collect::<Vec<_>>();
        let threads = (0..config.threads())
            .map(|_| {
                MultiperspectivePerceptronThreadSnapshot::new(
                    &config,
                    max_global_history,
                    path_entries,
                    recency_entries,
                )
            })
            .collect();

        Ok(Self {
            table_entries,
            tables,
            threads,
            mpreds: vec![0; config.features().len()],
            theta: config.initial_theta,
            threshold_counter: 0,
            lookup_count: 0,
            update_count: 0,
            config,
        })
    }

    pub const fn config(&self) -> &MultiperspectivePerceptronConfig {
        &self.config
    }

    pub fn table_entries(&self) -> &[usize] {
        &self.table_entries
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }

    pub fn thread_snapshot(
        &self,
        cpu: CpuId,
    ) -> Result<&MultiperspectivePerceptronThreadSnapshot, MultiperspectivePerceptronError> {
        self.threads
            .get(cpu.get() as usize)
            .ok_or(MultiperspectivePerceptronError::UnknownThread { cpu })
    }

    pub fn predict(
        &mut self,
        cpu: CpuId,
        pc: Address,
        conditional: bool,
    ) -> Result<MultiperspectivePerceptronPrediction, MultiperspectivePerceptronError> {
        let thread_index = self.thread_index(cpu)?;
        self.lookup_count += 1;
        let thread_before = self.threads[thread_index].clone();
        self.predict_with_thread_snapshot_and_count(
            cpu,
            pc,
            conditional,
            thread_before,
            self.lookup_count,
        )
    }

    pub fn train(
        &mut self,
        history: &MultiperspectivePerceptronHistory,
        actual_taken: bool,
        target: Address,
    ) -> Result<MultiperspectivePerceptronTrainingUpdate, MultiperspectivePerceptronError> {
        let thread_index = self.thread_index(history.cpu())?;
        let mut filter_after = None;
        let mut trained = false;
        let mut feature_updates = Vec::new();

        if history.conditional() {
            let mut do_train = true;
            if let Some(filter_index) = history.filter_index {
                let filter = &mut self.threads[thread_index].filter_table[filter_index];
                if filter.always_not_taken_so_far() || filter.always_taken_so_far() {
                    do_train = false;
                }
                if actual_taken {
                    filter.seen_taken = true;
                } else {
                    filter.seen_untaken = true;
                }
                filter_after = Some(filter.clone());
            }

            if do_train {
                trained = self.train_weights(history, actual_taken, &mut feature_updates);
            }
        }

        let thread_before = self.threads[thread_index].clone();
        self.update_thread_history(thread_index, history.pc(), actual_taken, target);
        let thread_after = self.threads[thread_index].clone();
        self.update_count += 1;

        Ok(MultiperspectivePerceptronTrainingUpdate {
            cpu: history.cpu(),
            pc: history.pc(),
            actual_taken,
            predicted_taken: history.predicted_taken(),
            trained,
            feature_updates,
            filter_after,
            thread_before,
            thread_after,
            update_count: self.update_count,
        })
    }

    pub fn snapshot(&self) -> MultiperspectivePerceptronSnapshot {
        MultiperspectivePerceptronSnapshot {
            config: self.config.clone(),
            table_entries: self.table_entries.clone(),
            tables: self.tables.clone(),
            threads: self.threads.clone(),
            mpreds: self.mpreds.clone(),
            theta: self.theta,
            threshold_counter: self.threshold_counter,
            lookup_count: self.lookup_count,
            update_count: self.update_count,
        }
    }

    pub fn restore(
        &mut self,
        snapshot: &MultiperspectivePerceptronSnapshot,
    ) -> Result<(), MultiperspectivePerceptronError> {
        if self.config != snapshot.config {
            return Err(MultiperspectivePerceptronError::SnapshotConfigMismatch {
                expected: Box::new(self.config.clone()),
                actual: Box::new(snapshot.config.clone()),
            });
        }
        validate_snapshot_shape(&self.config, snapshot)?;
        self.table_entries.clone_from(&snapshot.table_entries);
        self.tables.clone_from(&snapshot.tables);
        self.threads.clone_from(&snapshot.threads);
        self.mpreds.clone_from(&snapshot.mpreds);
        self.theta = snapshot.theta;
        self.threshold_counter = snapshot.threshold_counter;
        self.lookup_count = snapshot.lookup_count;
        self.update_count = snapshot.update_count;
        Ok(())
    }

    fn train_weights(
        &mut self,
        history: &MultiperspectivePerceptronHistory,
        actual_taken: bool,
        feature_updates: &mut Vec<MultiperspectivePerceptronFeatureUpdate>,
    ) -> bool {
        let correct = history.predicted_taken() == actual_taken;
        let abs_sum = history.linear_sum().abs();
        self.update_feature_mispred_counts(history, actual_taken, abs_sum);
        if correct && abs_sum > self.theta {
            return false;
        }

        if !correct {
            self.threshold_counter += 1;
            if self.threshold_counter >= self.config.speed {
                self.theta += 1;
                self.threshold_counter = 0;
            }
        } else if abs_sum < self.theta {
            self.threshold_counter -= 1;
            if self.threshold_counter <= -self.config.speed {
                self.theta = self.theta.saturating_sub(1);
                self.threshold_counter = 0;
            }
        }

        for (feature_index, table_index) in history.feature_indices().iter().enumerate() {
            let feature = &self.config.features[feature_index];
            let sign_slot = self.sign_slot(history.hpc(), feature_index);
            let weight = &mut self.tables[feature_index][*table_index];
            let sign_before = weight.sign_bits[sign_slot];
            let magnitude_before = weight.magnitude;
            sat_inc_dec(
                actual_taken,
                &mut weight.sign_bits[sign_slot],
                &mut weight.magnitude,
                max_magnitude(feature.width),
            );
            feature_updates.push(MultiperspectivePerceptronFeatureUpdate {
                feature_index,
                table_index: *table_index,
                sign_before,
                sign_after: weight.sign_bits[sign_slot],
                magnitude_before,
                magnitude_after: weight.magnitude,
            });
        }

        true
    }

    fn update_feature_mispred_counts(
        &mut self,
        history: &MultiperspectivePerceptronHistory,
        actual_taken: bool,
        abs_sum: i16,
    ) {
        if self.config.threshold < 0 {
            return;
        }
        if self.config.tune_only && abs_sum > self.config.threshold {
            return;
        }
        let max = if self.config.tune_bits >= 63 {
            u64::MAX
        } else {
            (1u64 << self.config.tune_bits) - 1
        };
        let mut halve = false;
        for (feature_index, value) in history.feature_values().iter().enumerate() {
            let feature_prediction = *value >= 1;
            if feature_prediction != actual_taken {
                self.mpreds[feature_index] = self.mpreds[feature_index].saturating_add(1);
                if self.mpreds[feature_index] >= max {
                    halve = true;
                }
            }
        }
        if halve {
            for value in &mut self.mpreds {
                *value /= 2;
            }
        }
    }

    fn update_thread_history(
        &mut self,
        thread_index: usize,
        pc: Address,
        taken: bool,
        target: Address,
    ) {
        let thread = &mut self.threads[thread_index];
        Self::update_snapshot_thread_history(&self.config, thread, pc, taken, target);
    }

    fn compute_output(
        &self,
        thread: &MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
        pc2: u16,
        hpc: u16,
    ) -> MultiperspectivePerceptronOutput {
        let mut linear_sum = self.local_history_bias(thread, pc);
        let mut best_sum = 0;
        let best_features = self.best_features();
        let mut feature_indices = Vec::with_capacity(self.config.features.len());
        let mut feature_values = Vec::with_capacity(self.config.features.len());

        for (feature_index, feature) in self.config.features.iter().enumerate() {
            let table_index = self.feature_index(thread, pc, pc2, hpc, feature_index, feature);
            let weight = self.signed_weight(feature_index, table_index, hpc);
            let value = (weight * feature.coefficient_q6) / 64;
            linear_sum += value;
            if best_features.contains(&feature_index) {
                best_sum += value;
            }
            feature_indices.push(table_index);
            feature_values.push(value);
        }

        linear_sum = ((linear_sum as i32 * self.config.fudge_q6 as i32) / 64) as i16;
        best_sum = ((best_sum as i32 * self.config.fudge_q6 as i32) / 64) as i16;

        MultiperspectivePerceptronOutput {
            linear_sum,
            best_sum,
            feature_indices,
            feature_values,
        }
    }

    fn feature_indices(
        &self,
        thread: &MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
        pc2: u16,
        hpc: u16,
    ) -> Vec<usize> {
        self.config
            .features
            .iter()
            .enumerate()
            .map(|(feature_index, feature)| {
                self.feature_index(thread, pc, pc2, hpc, feature_index, feature)
            })
            .collect()
    }

    fn feature_index(
        &self,
        thread: &MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
        pc2: u16,
        hpc: u16,
        feature_index: usize,
        feature: &MultiperspectivePerceptronFeature,
    ) -> usize {
        let mut hash = self.feature_hash(thread, pc, pc2, feature) as u64;
        if self.config.hshift < 0 {
            hash = (hash << (-self.config.hshift as u8)) ^ u64::from(pc2);
        } else {
            hash = (hash << (self.config.hshift as u8)) ^ u64::from(hpc);
        }
        let feature_mask = bit_for_feature(feature_index);
        if self.config.imli_mask1 & feature_mask != 0 {
            hash ^= u64::from(thread.imli_counters[0]);
        }
        if self.config.imli_mask4 & feature_mask != 0 {
            hash ^= u64::from(thread.imli_counters[3]);
        }
        if self.config.recencypos_mask & feature_mask != 0 {
            hash ^= u64::from(thread.recency_position(pc2) as u16);
        }
        (hash as usize) % self.table_entries[feature_index]
    }

    fn feature_hash(
        &self,
        thread: &MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
        pc2: u16,
        feature: &MultiperspectivePerceptronFeature,
    ) -> u32 {
        match feature.kind {
            MultiperspectivePerceptronFeatureKind::Bias => u32::from(pc2),
            MultiperspectivePerceptronFeatureKind::GlobalHistory => {
                thread.global_history_hash(feature.p1, feature.p2)
            }
            MultiperspectivePerceptronFeatureKind::GlobalHistoryPath => {
                thread.global_history_hash(feature.p1, feature.p1 + feature.p2)
                    ^ thread.path_hash(feature.p2.max(1) as usize, feature.p3)
            }
            MultiperspectivePerceptronFeatureKind::GlobalHistoryModuloPath => {
                let path = thread
                    .path_history
                    .iter()
                    .take(feature.p2.max(1) as usize)
                    .fold(0u32, |hash, pc| {
                        hash.rotate_left(3) ^ (u32::from(*pc) >> feature.p3.max(0) as u32)
                    });
                thread.global_history_hash(0, feature.p2) ^ path ^ feature.p1.max(0) as u32
            }
            MultiperspectivePerceptronFeatureKind::Imli => {
                let index = feature.p1.clamp(0, 3) as usize;
                u32::from(thread.imli_counters[index])
            }
            MultiperspectivePerceptronFeatureKind::Local => thread.local_history_for(pc) as u32,
            MultiperspectivePerceptronFeatureKind::Recency => {
                thread.recency_hash(feature.p1.max(1) as usize, feature.p2)
            }
            MultiperspectivePerceptronFeatureKind::RecencyPosition => {
                thread.recency_position(pc2) as u32
            }
            MultiperspectivePerceptronFeatureKind::ShiftedGlobalHistoryPath => {
                (thread.global_history_hash(feature.p1, feature.p1 + 16)
                    << feature.p3.max(0) as u32)
                    ^ thread.path_hash(feature.p2.max(1) as usize, feature.p3)
            }
            MultiperspectivePerceptronFeatureKind::Acyclic => {
                thread.global_history_hash(0, feature.p1 + 2) ^ u32::from(pc2)
            }
            MultiperspectivePerceptronFeatureKind::BlurryPath => {
                (pc.get() as u32 >> feature.p1.max(0) as u32)
                    ^ thread.path_hash(feature.p2.max(1) as usize, feature.p3)
            }
            MultiperspectivePerceptronFeatureKind::ModuloHistory => {
                thread.global_history_hash(0, feature.p2) ^ feature.p1.max(0) as u32
            }
            MultiperspectivePerceptronFeatureKind::ModuloPath
            | MultiperspectivePerceptronFeatureKind::Path => {
                thread.path_hash(feature.p2.max(1) as usize, feature.p3)
            }
        }
    }

    fn local_history_bias(
        &self,
        thread: &MultiperspectivePerceptronThreadSnapshot,
        pc: Address,
    ) -> i16 {
        let local_history = thread.local_history_for(pc);
        let history_mask = bit_mask(self.config.local_history_length);
        if local_history == 0 {
            self.config.bias0
        } else if local_history == history_mask {
            self.config.bias1
        } else if local_history == (1u64 << (self.config.local_history_length - 1)) {
            self.config.bias_mostly0
        } else if local_history == ((1u64 << (self.config.local_history_length - 1)) - 1) {
            self.config.bias_mostly1
        } else {
            0
        }
    }

    fn signed_weight(&self, feature_index: usize, table_index: usize, hpc: u16) -> i16 {
        let feature = &self.config.features[feature_index];
        let weight = &self.tables[feature_index][table_index];
        let magnitude = translate_magnitude(weight.magnitude, feature.width);
        if weight.sign_bits[self.sign_slot(hpc, feature_index)] {
            -magnitude
        } else {
            magnitude
        }
    }

    fn sign_slot(&self, hpc: u16, _feature_index: usize) -> usize {
        usize::from(hpc) % self.config.n_sign_bits as usize
    }

    fn best_features(&self) -> Vec<usize> {
        let mut pairs = self
            .mpreds
            .iter()
            .enumerate()
            .map(|(index, value)| (Reverse(*value), index))
            .collect::<Vec<_>>();
        pairs.sort();
        pairs
            .into_iter()
            .take(self.config.nbest.min(self.config.features.len()))
            .map(|(_, index)| index)
            .collect()
    }

    fn filter_index(
        &self,
        thread: &MultiperspectivePerceptronThreadSnapshot,
        hpc: u16,
    ) -> Option<usize> {
        if thread.filter_table.is_empty() {
            None
        } else {
            Some(usize::from(hpc) % thread.filter_table.len())
        }
    }

    fn thread_index(&self, cpu: CpuId) -> Result<usize, MultiperspectivePerceptronError> {
        let index = cpu.get() as usize;
        if index >= self.threads.len() {
            return Err(MultiperspectivePerceptronError::UnknownThread { cpu });
        }
        Ok(index)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronPrediction {
    history: MultiperspectivePerceptronHistory,
}

impl MultiperspectivePerceptronPrediction {
    pub const fn history(&self) -> &MultiperspectivePerceptronHistory {
        &self.history
    }

    pub const fn predicted_taken(&self) -> bool {
        self.history.predicted_taken()
    }

    pub const fn filtered(&self) -> bool {
        self.history.filtered()
    }

    pub const fn used_static_prediction(&self) -> bool {
        self.history.used_static_prediction()
    }

    pub const fn linear_sum(&self) -> i16 {
        self.history.linear_sum()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronHistory {
    cpu: CpuId,
    pc: Address,
    conditional: bool,
    predicted_taken: bool,
    filtered: bool,
    used_static_prediction: bool,
    linear_sum: i16,
    best_sum: i16,
    feature_indices: Vec<usize>,
    feature_values: Vec<i16>,
    hpc: u16,
    filter_index: Option<usize>,
    filter_before: Option<MultiperspectivePerceptronFilterEntry>,
    thread_before: MultiperspectivePerceptronThreadSnapshot,
    lookup_count: u64,
}

impl MultiperspectivePerceptronHistory {
    fn unconditional(
        cpu: CpuId,
        pc: Address,
        hpc: u16,
        thread_before: MultiperspectivePerceptronThreadSnapshot,
        lookup_count: u64,
    ) -> Self {
        Self {
            cpu,
            pc,
            conditional: false,
            predicted_taken: true,
            filtered: false,
            used_static_prediction: false,
            linear_sum: 0,
            best_sum: 0,
            feature_indices: Vec::new(),
            feature_values: Vec::new(),
            hpc,
            filter_index: None,
            filter_before: None,
            thread_before,
            lookup_count,
        }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn pc(&self) -> Address {
        self.pc
    }

    pub const fn conditional(&self) -> bool {
        self.conditional
    }

    pub const fn predicted_taken(&self) -> bool {
        self.predicted_taken
    }

    pub const fn filtered(&self) -> bool {
        self.filtered
    }

    pub const fn used_static_prediction(&self) -> bool {
        self.used_static_prediction
    }

    pub const fn linear_sum(&self) -> i16 {
        self.linear_sum
    }

    pub fn feature_indices(&self) -> &[usize] {
        &self.feature_indices
    }

    pub fn feature_values(&self) -> &[i16] {
        &self.feature_values
    }

    pub const fn hpc(&self) -> u16 {
        self.hpc
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronTrainingUpdate {
    cpu: CpuId,
    pc: Address,
    actual_taken: bool,
    predicted_taken: bool,
    trained: bool,
    feature_updates: Vec<MultiperspectivePerceptronFeatureUpdate>,
    filter_after: Option<MultiperspectivePerceptronFilterEntry>,
    thread_before: MultiperspectivePerceptronThreadSnapshot,
    thread_after: MultiperspectivePerceptronThreadSnapshot,
    update_count: u64,
}

impl MultiperspectivePerceptronTrainingUpdate {
    pub const fn trained(&self) -> bool {
        self.trained
    }

    pub fn feature_updates(&self) -> &[MultiperspectivePerceptronFeatureUpdate] {
        &self.feature_updates
    }

    pub const fn filter_after(&self) -> Option<&MultiperspectivePerceptronFilterEntry> {
        self.filter_after.as_ref()
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronFeatureUpdate {
    feature_index: usize,
    table_index: usize,
    sign_before: bool,
    sign_after: bool,
    magnitude_before: u8,
    magnitude_after: u8,
}

impl MultiperspectivePerceptronFeatureUpdate {
    pub const fn magnitude_before(&self) -> u8 {
        self.magnitude_before
    }

    pub const fn magnitude_after(&self) -> u8 {
        self.magnitude_after
    }

    pub const fn sign_after(&self) -> bool {
        self.sign_after
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronThreadSnapshot {
    pub(crate) max_global_history: usize,
    pub(crate) max_path_entries: usize,
    pub(crate) filter_table: Vec<MultiperspectivePerceptronFilterEntry>,
    pub(crate) global_history: Vec<bool>,
    pub(crate) local_histories: Vec<u64>,
    pub(crate) path_history: Vec<u16>,
    pub(crate) recency_stack: Vec<u16>,
    pub(crate) imli_counters: [u16; 4],
    pub(crate) last_ghist_bit: bool,
}

impl MultiperspectivePerceptronThreadSnapshot {
    fn new(
        config: &MultiperspectivePerceptronConfig,
        max_global_history: usize,
        max_path_entries: usize,
        recency_entries: usize,
    ) -> Self {
        Self {
            max_global_history,
            max_path_entries,
            filter_table: vec![
                MultiperspectivePerceptronFilterEntry::default();
                config.num_filter_entries
            ],
            global_history: vec![false; max_global_history.max(config.initial_ghist_length)],
            local_histories: vec![0; config.num_local_histories],
            path_history: vec![0; max_path_entries.max(1)],
            recency_stack: vec![0; recency_entries],
            imli_counters: [0; 4],
            last_ghist_bit: false,
        }
    }

    pub fn global_history_prefix(&self, count: usize) -> Vec<bool> {
        self.global_history.iter().take(count).copied().collect()
    }

    pub fn local_history_for(&self, pc: Address) -> u64 {
        self.local_histories[self.local_index(pc)]
    }

    pub fn path_history(&self) -> &[u16] {
        &self.path_history
    }

    pub const fn imli_counters(&self) -> &[u16; 4] {
        &self.imli_counters
    }

    fn local_index(&self, pc: Address) -> usize {
        ((pc.get() >> 2) as usize) % self.local_histories.len()
    }

    fn insert_recency(&mut self, pc2: u16) {
        let position = self
            .recency_stack
            .iter()
            .position(|entry| *entry == pc2)
            .unwrap_or_else(|| self.recency_stack.len().saturating_sub(1));
        if self.recency_stack.is_empty() {
            return;
        }
        self.recency_stack[position] = pc2;
        let value = self.recency_stack[position];
        for index in (1..=position).rev() {
            self.recency_stack[index] = self.recency_stack[index - 1];
        }
        self.recency_stack[0] = value;
    }

    fn global_history_hash(&self, start: i16, end: i16) -> u32 {
        let start = start.max(0) as usize;
        let end = end.max(start as i16 + 1) as usize;
        let mut hash = 0u32;
        for bit in self
            .global_history
            .iter()
            .skip(start)
            .take(end.saturating_sub(start))
        {
            hash = hash.rotate_left(1) ^ u32::from(*bit);
        }
        hash
    }

    fn path_hash(&self, count: usize, shift: i16) -> u32 {
        self.path_history.iter().take(count).fold(0u32, |hash, pc| {
            let value = if shift < 0 {
                u32::from(*pc)
            } else {
                u32::from(*pc) >> shift as u32
            };
            hash.rotate_left(5) ^ value
        })
    }

    fn recency_hash(&self, count: usize, shift: i16) -> u32 {
        self.recency_stack
            .iter()
            .take(count.min(self.recency_stack.len()))
            .fold(0u32, |hash, pc| {
                let value = if shift < 0 {
                    u32::from(*pc)
                } else {
                    u32::from(*pc) >> shift as u32
                };
                hash.rotate_left(3) ^ value
            })
    }

    fn recency_position(&self, pc2: u16) -> usize {
        self.recency_stack
            .iter()
            .position(|entry| *entry == pc2)
            .unwrap_or(self.recency_stack.len())
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MultiperspectivePerceptronFilterEntry {
    pub(crate) seen_taken: bool,
    pub(crate) seen_untaken: bool,
}

impl MultiperspectivePerceptronFilterEntry {
    pub const fn seen_taken(&self) -> bool {
        self.seen_taken
    }

    pub const fn seen_untaken(&self) -> bool {
        self.seen_untaken
    }

    pub const fn always_not_taken_so_far(&self) -> bool {
        self.seen_untaken && !self.seen_taken
    }

    pub const fn always_taken_so_far(&self) -> bool {
        self.seen_taken && !self.seen_untaken
    }

    pub const fn never_seen(&self) -> bool {
        !self.seen_taken && !self.seen_untaken
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiperspectivePerceptronSnapshot {
    pub(crate) config: MultiperspectivePerceptronConfig,
    pub(crate) table_entries: Vec<usize>,
    pub(crate) tables: Vec<Vec<MultiperspectivePerceptronWeight>>,
    pub(crate) threads: Vec<MultiperspectivePerceptronThreadSnapshot>,
    pub(crate) mpreds: Vec<u64>,
    pub(crate) theta: i16,
    pub(crate) threshold_counter: i16,
    pub(crate) lookup_count: u64,
    pub(crate) update_count: u64,
}

impl MultiperspectivePerceptronSnapshot {
    pub const fn config(&self) -> &MultiperspectivePerceptronConfig {
        &self.config
    }

    pub const fn lookup_count(&self) -> u64 {
        self.lookup_count
    }

    pub const fn update_count(&self) -> u64 {
        self.update_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MultiperspectivePerceptronOutput {
    linear_sum: i16,
    best_sum: i16,
    feature_indices: Vec<usize>,
    feature_values: Vec<i16>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct MultiperspectivePerceptronWeight {
    pub(crate) magnitude: u8,
    pub(crate) sign_bits: Vec<bool>,
}

impl MultiperspectivePerceptronWeight {
    fn new(sign_bits: u8) -> Self {
        Self {
            magnitude: 0,
            sign_bits: vec![false; sign_bits as usize],
        }
    }
}

pub(crate) fn allocate_table_entries(
    config: &MultiperspectivePerceptronConfig,
) -> Result<Vec<usize>, MultiperspectivePerceptronError> {
    let fixed_bits = config
        .features
        .iter()
        .filter(|feature| feature.table_entries > 0)
        .map(|feature| feature.table_entries * bits_per_entry(feature, config))
        .sum::<usize>();
    if fixed_bits > config.budget_bits {
        return Err(MultiperspectivePerceptronError::BudgetTooSmall {
            budget_bits: config.budget_bits,
        });
    }
    let auto_features = config
        .features
        .iter()
        .filter(|feature| feature.table_entries == 0)
        .count();
    let auto_bits = if auto_features == 0 {
        0
    } else {
        (config.budget_bits - fixed_bits) / auto_features
    };

    Ok(config
        .features
        .iter()
        .map(|feature| {
            if feature.table_entries > 0 {
                feature.table_entries
            } else {
                (auto_bits / bits_per_entry(feature, config)).max(1)
            }
        })
        .collect())
}

fn bits_per_entry(
    feature: &MultiperspectivePerceptronFeature,
    config: &MultiperspectivePerceptronConfig,
) -> usize {
    feature.width as usize + config.n_sign_bits.saturating_sub(1) as usize
}

pub(crate) fn max_global_history(config: &MultiperspectivePerceptronConfig) -> usize {
    config
        .features
        .iter()
        .map(|feature| match feature.kind {
            MultiperspectivePerceptronFeatureKind::GlobalHistory => feature.p2.max(0) as usize + 1,
            MultiperspectivePerceptronFeatureKind::GlobalHistoryPath
            | MultiperspectivePerceptronFeatureKind::ShiftedGlobalHistoryPath => {
                (feature.p1 + feature.p2).max(0) as usize + 1
            }
            _ => config.initial_ghist_length,
        })
        .max()
        .unwrap_or(config.initial_ghist_length)
        .max(config.initial_ghist_length)
}

pub(crate) fn max_path_entries(config: &MultiperspectivePerceptronConfig) -> usize {
    if config.ignore_path_size {
        return 1;
    }
    config
        .features
        .iter()
        .map(|feature| match feature.kind {
            MultiperspectivePerceptronFeatureKind::GlobalHistoryPath
            | MultiperspectivePerceptronFeatureKind::GlobalHistoryModuloPath
            | MultiperspectivePerceptronFeatureKind::ShiftedGlobalHistoryPath
            | MultiperspectivePerceptronFeatureKind::Path
            | MultiperspectivePerceptronFeatureKind::ModuloPath => feature.p2.max(1) as usize,
            _ => 1,
        })
        .max()
        .unwrap_or(1)
}

pub(crate) fn max_recency_entries(config: &MultiperspectivePerceptronConfig) -> usize {
    config
        .features
        .iter()
        .filter_map(|feature| match feature.kind {
            MultiperspectivePerceptronFeatureKind::Recency
            | MultiperspectivePerceptronFeatureKind::RecencyPosition => {
                Some(feature.p1.max(1) as usize + 1)
            }
            _ => None,
        })
        .max()
        .unwrap_or(0)
}

fn max_magnitude(width: u8) -> u8 {
    (1u8 << (width - 1)) - 1
}

fn translate_magnitude(magnitude: u8, width: u8) -> i16 {
    if width == 5 {
        XLAT_5[magnitude.min((XLAT_5.len() - 1) as u8) as usize]
    } else {
        XLAT_6[magnitude.min((XLAT_6.len() - 1) as u8) as usize]
    }
}

fn sat_inc_dec(taken: bool, sign: &mut bool, magnitude: &mut u8, max: u8) {
    if taken {
        if *sign {
            if *magnitude == 0 {
                *sign = false;
            } else {
                *magnitude -= 1;
            }
        } else if *magnitude < max {
            *magnitude += 1;
        }
    } else if *sign {
        if *magnitude < max {
            *magnitude += 1;
        }
    } else if *magnitude == 0 {
        *sign = true;
    } else {
        *magnitude -= 1;
    }
}

fn pc2(pc: Address) -> u16 {
    (pc.get() >> 2) as u16
}

fn bit_mask(bits: u8) -> u64 {
    if bits >= u64::BITS as u8 {
        u64::MAX
    } else {
        (1u64 << bits) - 1
    }
}

fn bit_for_feature(feature_index: usize) -> u64 {
    if feature_index >= u64::BITS as usize {
        0
    } else {
        1u64 << feature_index
    }
}

fn hash_pc(pc: u32, pc_shift: i8) -> u16 {
    hash_pc_const(pc, pc_shift) as u16
}

const fn hash_pc_const(pc: u32, pc_shift: i8) -> u32 {
    if pc_shift < 0 {
        hash2(pc)
            .wrapping_mul((-pc_shift) as u32)
            .wrapping_add(hash1(pc))
    } else if pc_shift < 11 {
        pc ^ (pc >> pc_shift)
    } else {
        pc >> (pc_shift - 11)
    }
}

const fn hash1(mut value: u32) -> u32 {
    value = (value ^ 0xdeadbeef).wrapping_add(value << 4);
    value ^= value >> 10;
    value = value.wrapping_add(value << 7);
    value ^= value >> 13;
    value
}

const fn hash2(mut value: u32) -> u32 {
    value = (value ^ 61) ^ (value >> 16);
    value = value.wrapping_add(value << 3);
    value ^= value >> 4;
    value = value.wrapping_mul(0x27d4eb2d);
    value ^= value >> 15;
    value
}
