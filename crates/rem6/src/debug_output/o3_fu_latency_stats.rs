use rem6_cpu::O3RuntimeFuLatencyClass;

const fn average_ticks(total: u64, samples: u64) -> u64 {
    if samples == 0 {
        0
    } else {
        total / samples
    }
}

const fn min_latency_ticks(current: Option<u64>, latency: u64) -> Option<u64> {
    Some(match current {
        Some(current) => {
            if current < latency {
                current
            } else {
                latency
            }
        }
        None => latency,
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3FuLatencyClassTotals {
    pub(super) instructions: u64,
    pub(super) cycles: u64,
    pub(super) max_cycles: u64,
    min_cycles: Option<u64>,
}

impl Rem6O3FuLatencyClassTotals {
    pub(super) fn add(&mut self, latency: u64) {
        self.instructions = self.instructions.saturating_add(1);
        self.cycles = self.cycles.saturating_add(latency);
        self.max_cycles = self.max_cycles.max(latency);
        self.min_cycles = min_latency_ticks(self.min_cycles, latency);
    }

    pub(super) fn min_cycles_value(self) -> u64 {
        self.min_cycles.unwrap_or(0)
    }

    pub(super) fn avg_cycles(self) -> u64 {
        average_ticks(self.cycles, self.instructions)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Rem6O3FuLatencyClassStatNames {
    pub(super) class: O3RuntimeFuLatencyClass,
    pub(super) instructions: &'static str,
    pub(super) cycles: &'static str,
    pub(super) max_cycles: &'static str,
    pub(super) min_cycles: &'static str,
    pub(super) avg_cycles: &'static str,
}

pub(super) const REM6_O3_FU_LATENCY_CLASS_STATS: [Rem6O3FuLatencyClassStatNames;
    O3RuntimeFuLatencyClass::COUNT] = [
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarIntegerMul,
        instructions: "event.fu_integer_mul_instructions",
        cycles: "event.fu_integer_mul_latency_cycles",
        max_cycles: "event.fu_integer_mul_latency_max_cycles",
        min_cycles: "event.fu_integer_mul_latency_min_cycles",
        avg_cycles: "event.fu_integer_mul_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarIntegerDiv,
        instructions: "event.fu_integer_div_instructions",
        cycles: "event.fu_integer_div_latency_cycles",
        max_cycles: "event.fu_integer_div_latency_max_cycles",
        min_cycles: "event.fu_integer_div_latency_min_cycles",
        avg_cycles: "event.fu_integer_div_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatAdd,
        instructions: "event.fu_float_add_instructions",
        cycles: "event.fu_float_add_latency_cycles",
        max_cycles: "event.fu_float_add_latency_max_cycles",
        min_cycles: "event.fu_float_add_latency_min_cycles",
        avg_cycles: "event.fu_float_add_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatCompare,
        instructions: "event.fu_float_compare_instructions",
        cycles: "event.fu_float_compare_latency_cycles",
        max_cycles: "event.fu_float_compare_latency_max_cycles",
        min_cycles: "event.fu_float_compare_latency_min_cycles",
        avg_cycles: "event.fu_float_compare_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatMisc,
        instructions: "event.fu_float_misc_instructions",
        cycles: "event.fu_float_misc_latency_cycles",
        max_cycles: "event.fu_float_misc_latency_max_cycles",
        min_cycles: "event.fu_float_misc_latency_min_cycles",
        avg_cycles: "event.fu_float_misc_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatMul,
        instructions: "event.fu_float_mul_instructions",
        cycles: "event.fu_float_mul_latency_cycles",
        max_cycles: "event.fu_float_mul_latency_max_cycles",
        min_cycles: "event.fu_float_mul_latency_min_cycles",
        avg_cycles: "event.fu_float_mul_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatFma,
        instructions: "event.fu_float_fma_instructions",
        cycles: "event.fu_float_fma_latency_cycles",
        max_cycles: "event.fu_float_fma_latency_max_cycles",
        min_cycles: "event.fu_float_fma_latency_min_cycles",
        avg_cycles: "event.fu_float_fma_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatDiv,
        instructions: "event.fu_float_div_instructions",
        cycles: "event.fu_float_div_latency_cycles",
        max_cycles: "event.fu_float_div_latency_max_cycles",
        min_cycles: "event.fu_float_div_latency_min_cycles",
        avg_cycles: "event.fu_float_div_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::ScalarFloatSqrt,
        instructions: "event.fu_float_sqrt_instructions",
        cycles: "event.fu_float_sqrt_latency_cycles",
        max_cycles: "event.fu_float_sqrt_latency_max_cycles",
        min_cycles: "event.fu_float_sqrt_latency_min_cycles",
        avg_cycles: "event.fu_float_sqrt_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorIntegerMul,
        instructions: "event.fu_vector_integer_mul_instructions",
        cycles: "event.fu_vector_integer_mul_latency_cycles",
        max_cycles: "event.fu_vector_integer_mul_latency_max_cycles",
        min_cycles: "event.fu_vector_integer_mul_latency_min_cycles",
        avg_cycles: "event.fu_vector_integer_mul_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorIntegerDiv,
        instructions: "event.fu_vector_integer_div_instructions",
        cycles: "event.fu_vector_integer_div_latency_cycles",
        max_cycles: "event.fu_vector_integer_div_latency_max_cycles",
        min_cycles: "event.fu_vector_integer_div_latency_min_cycles",
        avg_cycles: "event.fu_vector_integer_div_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatAdd,
        instructions: "event.fu_vector_float_add_instructions",
        cycles: "event.fu_vector_float_add_latency_cycles",
        max_cycles: "event.fu_vector_float_add_latency_max_cycles",
        min_cycles: "event.fu_vector_float_add_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_add_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatCompare,
        instructions: "event.fu_vector_float_compare_instructions",
        cycles: "event.fu_vector_float_compare_latency_cycles",
        max_cycles: "event.fu_vector_float_compare_latency_max_cycles",
        min_cycles: "event.fu_vector_float_compare_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_compare_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatMisc,
        instructions: "event.fu_vector_float_misc_instructions",
        cycles: "event.fu_vector_float_misc_latency_cycles",
        max_cycles: "event.fu_vector_float_misc_latency_max_cycles",
        min_cycles: "event.fu_vector_float_misc_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_misc_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatMul,
        instructions: "event.fu_vector_float_mul_instructions",
        cycles: "event.fu_vector_float_mul_latency_cycles",
        max_cycles: "event.fu_vector_float_mul_latency_max_cycles",
        min_cycles: "event.fu_vector_float_mul_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_mul_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatFma,
        instructions: "event.fu_vector_float_fma_instructions",
        cycles: "event.fu_vector_float_fma_latency_cycles",
        max_cycles: "event.fu_vector_float_fma_latency_max_cycles",
        min_cycles: "event.fu_vector_float_fma_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_fma_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatDiv,
        instructions: "event.fu_vector_float_div_instructions",
        cycles: "event.fu_vector_float_div_latency_cycles",
        max_cycles: "event.fu_vector_float_div_latency_max_cycles",
        min_cycles: "event.fu_vector_float_div_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_div_latency_avg_cycles",
    },
    Rem6O3FuLatencyClassStatNames {
        class: O3RuntimeFuLatencyClass::VectorFloatSqrt,
        instructions: "event.fu_vector_float_sqrt_instructions",
        cycles: "event.fu_vector_float_sqrt_latency_cycles",
        max_cycles: "event.fu_vector_float_sqrt_latency_max_cycles",
        min_cycles: "event.fu_vector_float_sqrt_latency_min_cycles",
        avg_cycles: "event.fu_vector_float_sqrt_latency_avg_cycles",
    },
];
