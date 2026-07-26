use super::*;

fn public_method_body<'a>(source: &'a str, method: &str) -> &'a str {
    let signature = format!("pub fn {method}(");
    let method_start = source.find(&signature).unwrap_or_else(|| {
        panic!("src/riscv_translation.rs is missing future public method `{method}`")
    });
    let open_offset = source[method_start..]
        .find('{')
        .unwrap_or_else(|| panic!("public method `{method}` is missing its opening brace"));
    let body_start = method_start + open_offset + 1;
    let mut depth = 1_usize;

    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &source[body_start..body_start + offset];
                }
            }
            _ => {}
        }
    }

    panic!("public method `{method}` is missing its balanced closing brace");
}

#[test]
fn riscv_vector_snapshot_and_restore_use_one_lock_and_checker_sync() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = rust_code_without_comments_and_literals(
        &fs::read_to_string(crate_dir.join("src/riscv_translation.rs")).unwrap(),
    );
    let snapshot = compact_rust_code(public_method_body(&source, "vector_architectural_state"));
    let restore = compact_rust_code(public_method_body(
        &source,
        "restore_vector_architectural_state",
    ));

    for (name, body) in [
        ("vector_architectural_state", snapshot.as_str()),
        ("restore_vector_architectural_state", restore.as_str()),
    ] {
        assert_eq!(
            body.matches(".lock()").count(),
            1,
            "RiscvCore::{name} must hold the core-state lock exactly once"
        );
    }

    assert_eq!(
        snapshot.matches("hart.vector_architectural_state(").count(),
        1,
        "RiscvCore snapshot must delegate exactly once to the complete hart snapshot"
    );
    for forbidden in [
        "read_vector_register",
        "read_vector(",
        "RiscvVectorArchitecturalState::new",
        "vector_config(",
        "vector_fixed_point(",
    ] {
        assert!(
            !snapshot.contains(forbidden),
            "RiscvCore snapshot must not rebuild vector state through `{forbidden}`"
        );
    }

    assert_eq!(
        restore
            .matches("hart.restore_vector_architectural_state(")
            .count(),
        1,
        "RiscvCore restore must delegate exactly once to the complete hart restore"
    );
    assert_eq!(
        restore.matches("riscv_checker::sync_checker_hart(").count(),
        1,
        "RiscvCore restore must synchronize the checker exactly once"
    );
    for forbidden in [
        "write_vector_register",
        "write_vector(",
        "set_vector_config",
        "set_vector_fixed_point",
    ] {
        assert!(
            !restore.contains(forbidden),
            "RiscvCore restore must not replay vector fields through `{forbidden}`"
        );
    }
}

#[test]
fn vector_checkpoint_method_extraction_ignores_non_code_structure() {
    let source = r####"
// pub fn vector_architectural_state() { fake_comment_call(); }
const NORMAL: &str = "pub fn vector_architectural_state() { fake_string_call(); }";
const RAW: &str = r##"pub fn vector_architectural_state() { fake_raw_call(); }"##;

pub fn vector_architectural_state(&self) -> usize {
    // fake_comment_call(); }
    let _normal = "fake_string_call(); }";
    let _raw = r#"fake_raw_call(); }"#;
    let _character = '}';
    let answer = if true { 1 } else { 2 };
    answer
}
"####;
    let code = rust_code_without_comments_and_literals(source);
    let body = compact_rust_code(public_method_body(&code, "vector_architectural_state"));

    assert!(body.contains("letanswer=iftrue{1}else{2};answer"));
    for fake in ["fake_comment_call", "fake_string_call", "fake_raw_call"] {
        assert!(
            !body.contains(fake),
            "masked method extraction admitted non-code `{fake}`"
        );
    }
}
