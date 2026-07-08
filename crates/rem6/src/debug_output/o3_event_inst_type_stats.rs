use rem6_cpu::O3RuntimeFuLatencyClass;

pub(super) const fn o3_event_iq_issued_inst_type_stat_suffix(
    class: O3RuntimeFuLatencyClass,
) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "event.iq_issued_inst_type.int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "event.iq_issued_inst_type.int_div",
        O3RuntimeFuLatencyClass::ScalarFloatAdd => "event.iq_issued_inst_type.float_add",
        O3RuntimeFuLatencyClass::ScalarFloatCompare => "event.iq_issued_inst_type.float_compare",
        O3RuntimeFuLatencyClass::ScalarFloatMisc => "event.iq_issued_inst_type.float_misc",
        O3RuntimeFuLatencyClass::ScalarFloatMul => "event.iq_issued_inst_type.float_mul",
        O3RuntimeFuLatencyClass::ScalarFloatFma => "event.iq_issued_inst_type.float_fma",
        O3RuntimeFuLatencyClass::ScalarFloatDiv => "event.iq_issued_inst_type.float_div",
        O3RuntimeFuLatencyClass::ScalarFloatSqrt => "event.iq_issued_inst_type.float_sqrt",
        O3RuntimeFuLatencyClass::VectorIntegerMul => "event.iq_issued_inst_type.vector_integer_mul",
        O3RuntimeFuLatencyClass::VectorIntegerDiv => "event.iq_issued_inst_type.vector_integer_div",
        O3RuntimeFuLatencyClass::VectorFloatAdd => "event.iq_issued_inst_type.vector_float_add",
        O3RuntimeFuLatencyClass::VectorFloatCompare => {
            "event.iq_issued_inst_type.vector_float_compare"
        }
        O3RuntimeFuLatencyClass::VectorFloatMisc => "event.iq_issued_inst_type.vector_float_misc",
        O3RuntimeFuLatencyClass::VectorFloatMul => "event.iq_issued_inst_type.vector_float_mul",
        O3RuntimeFuLatencyClass::VectorFloatFma => "event.iq_issued_inst_type.vector_float_fma",
        O3RuntimeFuLatencyClass::VectorFloatDiv => "event.iq_issued_inst_type.vector_float_div",
        O3RuntimeFuLatencyClass::VectorFloatSqrt => "event.iq_issued_inst_type.vector_float_sqrt",
    }
}

pub(super) const fn o3_event_commit_committed_inst_type_stat_suffix(
    class: O3RuntimeFuLatencyClass,
) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "event.commit_committed_inst_type.int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "event.commit_committed_inst_type.int_div",
        O3RuntimeFuLatencyClass::ScalarFloatAdd => "event.commit_committed_inst_type.float_add",
        O3RuntimeFuLatencyClass::ScalarFloatCompare => {
            "event.commit_committed_inst_type.float_compare"
        }
        O3RuntimeFuLatencyClass::ScalarFloatMisc => "event.commit_committed_inst_type.float_misc",
        O3RuntimeFuLatencyClass::ScalarFloatMul => "event.commit_committed_inst_type.float_mul",
        O3RuntimeFuLatencyClass::ScalarFloatFma => "event.commit_committed_inst_type.float_fma",
        O3RuntimeFuLatencyClass::ScalarFloatDiv => "event.commit_committed_inst_type.float_div",
        O3RuntimeFuLatencyClass::ScalarFloatSqrt => "event.commit_committed_inst_type.float_sqrt",
        O3RuntimeFuLatencyClass::VectorIntegerMul => {
            "event.commit_committed_inst_type.vector_integer_mul"
        }
        O3RuntimeFuLatencyClass::VectorIntegerDiv => {
            "event.commit_committed_inst_type.vector_integer_div"
        }
        O3RuntimeFuLatencyClass::VectorFloatAdd => {
            "event.commit_committed_inst_type.vector_float_add"
        }
        O3RuntimeFuLatencyClass::VectorFloatCompare => {
            "event.commit_committed_inst_type.vector_float_compare"
        }
        O3RuntimeFuLatencyClass::VectorFloatMisc => {
            "event.commit_committed_inst_type.vector_float_misc"
        }
        O3RuntimeFuLatencyClass::VectorFloatMul => {
            "event.commit_committed_inst_type.vector_float_mul"
        }
        O3RuntimeFuLatencyClass::VectorFloatFma => {
            "event.commit_committed_inst_type.vector_float_fma"
        }
        O3RuntimeFuLatencyClass::VectorFloatDiv => {
            "event.commit_committed_inst_type.vector_float_div"
        }
        O3RuntimeFuLatencyClass::VectorFloatSqrt => {
            "event.commit_committed_inst_type.vector_float_sqrt"
        }
    }
}
