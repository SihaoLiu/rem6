use std::collections::BTreeSet;

use rem6_cpu::{
    O3RuntimeFuLatencyClass, O3RuntimeInstTypeDescriptor, O3_RUNTIME_INST_TYPE_DESCRIPTORS,
};

fn descriptor_tuple(
    descriptor: &O3RuntimeInstTypeDescriptor,
) -> (
    O3RuntimeFuLatencyClass,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    bool,
) {
    (
        descriptor.class(),
        descriptor.source_stem(),
        descriptor.gem5_alias(),
        descriptor.event_iq_stat_suffix(),
        descriptor.event_commit_stat_suffix(),
        descriptor.zero_extended_alias(),
    )
}

#[test]
fn o3_runtime_inst_type_descriptors_follow_fu_latency_class_order() {
    let expected = [
        (
            O3RuntimeFuLatencyClass::ScalarIntegerMul,
            "int_mul",
            "IntMult",
            "event.iq_issued_inst_type.int_mul",
            "event.commit_committed_inst_type.int_mul",
            false,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarIntegerDiv,
            "int_div",
            "IntDiv",
            "event.iq_issued_inst_type.int_div",
            "event.commit_committed_inst_type.int_div",
            false,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatAdd,
            "float_add",
            "FloatAdd",
            "event.iq_issued_inst_type.float_add",
            "event.commit_committed_inst_type.float_add",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatCompare,
            "float_compare",
            "FloatCmp",
            "event.iq_issued_inst_type.float_compare",
            "event.commit_committed_inst_type.float_compare",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatMisc,
            "float_misc",
            "FloatMisc",
            "event.iq_issued_inst_type.float_misc",
            "event.commit_committed_inst_type.float_misc",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatMul,
            "float_mul",
            "FloatMult",
            "event.iq_issued_inst_type.float_mul",
            "event.commit_committed_inst_type.float_mul",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatFma,
            "float_fma",
            "FloatMultAcc",
            "event.iq_issued_inst_type.float_fma",
            "event.commit_committed_inst_type.float_fma",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatDiv,
            "float_div",
            "FloatDiv",
            "event.iq_issued_inst_type.float_div",
            "event.commit_committed_inst_type.float_div",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::ScalarFloatSqrt,
            "float_sqrt",
            "FloatSqrt",
            "event.iq_issued_inst_type.float_sqrt",
            "event.commit_committed_inst_type.float_sqrt",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorIntegerMul,
            "vector_integer_mul",
            "SimdMult",
            "event.iq_issued_inst_type.vector_integer_mul",
            "event.commit_committed_inst_type.vector_integer_mul",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorIntegerDiv,
            "vector_integer_div",
            "SimdDiv",
            "event.iq_issued_inst_type.vector_integer_div",
            "event.commit_committed_inst_type.vector_integer_div",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatAdd,
            "vector_float_add",
            "SimdFloatAdd",
            "event.iq_issued_inst_type.vector_float_add",
            "event.commit_committed_inst_type.vector_float_add",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatCompare,
            "vector_float_compare",
            "SimdFloatCmp",
            "event.iq_issued_inst_type.vector_float_compare",
            "event.commit_committed_inst_type.vector_float_compare",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatMisc,
            "vector_float_misc",
            "SimdFloatMisc",
            "event.iq_issued_inst_type.vector_float_misc",
            "event.commit_committed_inst_type.vector_float_misc",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatMul,
            "vector_float_mul",
            "SimdFloatMult",
            "event.iq_issued_inst_type.vector_float_mul",
            "event.commit_committed_inst_type.vector_float_mul",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatFma,
            "vector_float_fma",
            "SimdFloatMultAcc",
            "event.iq_issued_inst_type.vector_float_fma",
            "event.commit_committed_inst_type.vector_float_fma",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatDiv,
            "vector_float_div",
            "SimdFloatDiv",
            "event.iq_issued_inst_type.vector_float_div",
            "event.commit_committed_inst_type.vector_float_div",
            true,
        ),
        (
            O3RuntimeFuLatencyClass::VectorFloatSqrt,
            "vector_float_sqrt",
            "SimdFloatSqrt",
            "event.iq_issued_inst_type.vector_float_sqrt",
            "event.commit_committed_inst_type.vector_float_sqrt",
            true,
        ),
    ];

    assert_eq!(O3_RUNTIME_INST_TYPE_DESCRIPTORS.len(), 18);
    assert_eq!(O3RuntimeFuLatencyClass::ALL.len(), 18);
    assert_eq!(expected.len(), 18);

    for (index, expected_row) in expected.iter().enumerate() {
        let class = expected_row.0;
        let descriptor = &O3_RUNTIME_INST_TYPE_DESCRIPTORS[index];

        assert_eq!(O3RuntimeFuLatencyClass::ALL[index], class);
        assert_eq!(class.index(), index);
        assert_eq!(descriptor_tuple(descriptor), *expected_row);
        assert_eq!(class.inst_type_descriptor(), descriptor);
    }
}

#[test]
fn o3_runtime_inst_type_descriptor_alias_fields_are_unique() {
    let mut source_stems = BTreeSet::new();
    let mut gem5_aliases = BTreeSet::new();
    let mut iq_suffixes = BTreeSet::new();
    let mut commit_suffixes = BTreeSet::new();

    for descriptor in O3_RUNTIME_INST_TYPE_DESCRIPTORS.iter() {
        assert!(
            source_stems.insert(descriptor.source_stem()),
            "duplicate O3 inst-type source stem `{}`",
            descriptor.source_stem()
        );
        assert!(
            gem5_aliases.insert(descriptor.gem5_alias()),
            "duplicate O3 inst-type gem5 alias `{}`",
            descriptor.gem5_alias()
        );
        assert!(
            iq_suffixes.insert(descriptor.event_iq_stat_suffix()),
            "duplicate O3 inst-type IQ suffix `{}`",
            descriptor.event_iq_stat_suffix()
        );
        assert!(
            commit_suffixes.insert(descriptor.event_commit_stat_suffix()),
            "duplicate O3 inst-type commit suffix `{}`",
            descriptor.event_commit_stat_suffix()
        );
    }

    let zero_extended_false = O3_RUNTIME_INST_TYPE_DESCRIPTORS
        .iter()
        .filter(|descriptor| !descriptor.zero_extended_alias())
        .map(|descriptor| descriptor.class())
        .collect::<Vec<_>>();
    assert_eq!(
        zero_extended_false.as_slice(),
        &[
            O3RuntimeFuLatencyClass::ScalarIntegerMul,
            O3RuntimeFuLatencyClass::ScalarIntegerDiv,
        ]
    );
}
