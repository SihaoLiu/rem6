use super::*;

const POLICY: &str = "tests/source_policy/live_checkpoint.rs";
const LEDGER: &str = "docs/architecture/gem5-to-rem6-migration.md";
const CPU_HEADING: &str = "### CPU Execution Models - 74% representative";
const CPU_SCORE: &str = "**Score calculation:** 8 of 10 items have executable evidence, or 80% raw, capped at the 74% representative bucket cap.";
const BOUNDED_CLAIM: &str = "checkpoint-restorable compute IQ window plus exactly one response-admitted scalar FLW/FLD result";
const RETAINED_GAPS: &str = "Pre-response transport, general IQ shapes, broader memory/result state, and a general O3 engine remain non-restorable.";

#[test]
fn live_checkpoint_cpu_owners_are_unconditional_and_bounded() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let parent = fs::read_to_string(crate_dir.join("tests/source_policy.rs")).unwrap();
    let child = fs::read_to_string(crate_dir.join(POLICY)).unwrap();
    let owned = |parent: &str, child: &str| {
        active_unconditional_path_owned_module_declaration_count(
            parent,
            child,
            "source_policy/live_checkpoint.rs",
            "live_checkpoint",
        ) == 1
    };

    assert!(owned(&parent, &child));
    let attachment = "#[path = \"source_policy/live_checkpoint.rs\"]\nmod live_checkpoint;";
    for conditional in ["#[cfg(any())]\n", "#[cfg_attr(all(), cfg(any()))]\n"] {
        let mutated = parent.replacen(attachment, &format!("{conditional}{attachment}"), 1);
        assert_ne!(mutated, parent, "attachment mutation must apply");
        assert!(!owned(&mutated, &child));
    }

    for (relative, maximum) in [
        (POLICY, 500),
        ("src/riscv_live_checkpoint.rs", 900),
        ("src/riscv_live_checkpoint/codec.rs", 1_100),
        ("src/riscv_live_checkpoint/event.rs", 175),
        ("src/o3_runtime_live_checkpoint.rs", 1_350),
        ("src/riscv_core_checkpoint_restore.rs", 375),
        ("src/riscv_live_checkpoint_tests.rs", 200),
        ("src/riscv_live_checkpoint_tests/codec.rs", 300),
        ("src/riscv_live_checkpoint_tests/compute.rs", 1_050),
        ("src/riscv_live_checkpoint_tests/fp_result.rs", 1_050),
        ("src/riscv_live_checkpoint_tests/rejections.rs", 375),
    ] {
        let path = crate_dir.join(relative);
        let lines = line_count(&path);
        assert!(
            lines <= maximum,
            "{relative} has {lines} lines, exceeding its {maximum}-line cap"
        );
    }
}

#[test]
fn live_checkpoint_cpu_wire_contracts_are_distinct_and_versioned() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let live = fs::read_to_string(crate_dir.join("src/riscv_live_checkpoint.rs")).unwrap();
    let codec = fs::read_to_string(crate_dir.join("src/riscv_live_checkpoint/codec.rs")).unwrap();
    let runtime = fs::read_to_string(crate_dir.join("src/o3_runtime_checkpoint.rs")).unwrap();
    let pipeline = fs::read_to_string(crate_dir.join("src/o3_pipeline.rs")).unwrap();
    let handoff =
        fs::read_to_string(crate_dir.join("src/riscv_execution_mode_handoff/codec.rs")).unwrap();

    assert!(wire_contract(&live, &codec, &runtime, &pipeline, &handoff));

    let wrong_o3lc_legacy = codec.replacen(
        "const VERSION_LEGACY: u8 = 1;",
        "const VERSION_LEGACY: u8 = 0;",
        1,
    );
    assert_ne!(
        wrong_o3lc_legacy, codec,
        "O3LC legacy-version mutation must apply"
    );
    assert!(!wire_contract(
        &live,
        &wrong_o3lc_legacy,
        &runtime,
        &pipeline,
        &handoff,
    ));

    let wrong_o3lc_current = codec.replacen(
        "const VERSION_CURRENT: u8 = 2;",
        "const VERSION_CURRENT: u8 = 3;",
        1,
    );
    assert_ne!(
        wrong_o3lc_current, codec,
        "O3LC current-version mutation must apply"
    );
    assert!(!wire_contract(
        &live,
        &wrong_o3lc_current,
        &runtime,
        &pipeline,
        &handoff,
    ));

    let commented_magic = codec.replacen(
        "const MAGIC: [u8; 4] = *b\"O3LC\";",
        "/* const MAGIC: [u8; 4] = *b\"O3LC\"; */",
        1,
    );
    assert_ne!(commented_magic, codec, "O3LC magic mutation must apply");
    assert!(!wire_contract(
        &live,
        &commented_magic,
        &runtime,
        &pipeline,
        &handoff,
    ));

    let wrong_o3rt = runtime.replacen(
        "const O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 23;",
        "const O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS: u8 = 24;",
        1,
    );
    assert_ne!(wrong_o3rt, runtime, "O3RT version mutation must apply");
    assert!(!wire_contract(
        &live,
        &codec,
        &wrong_o3rt,
        &pipeline,
        &handoff,
    ));

    let wrong_o3ps = pipeline.replacen(
        "const O3_PENDING_STATE_CHECKPOINT_VERSION: u8 = 2;",
        "const O3_PENDING_STATE_CHECKPOINT_VERSION: u8 = 3;",
        1,
    );
    assert_ne!(wrong_o3ps, pipeline, "O3PS version mutation must apply");
    assert!(!wire_contract(
        &live,
        &codec,
        &runtime,
        &wrong_o3ps,
        &handoff,
    ));

    let wrong_o3dh = handoff.replacen(
        "pub(super) const VERSION_CURRENT: u8 = 7;",
        "pub(super) const VERSION_CURRENT: u8 = 8;",
        1,
    );
    assert_ne!(wrong_o3dh, handoff, "O3DH version mutation must apply");
    assert!(!wire_contract(
        &live,
        &codec,
        &runtime,
        &pipeline,
        &wrong_o3dh,
    ));

    let chunk_declaration =
        "pub const RISCV_O3_LIVE_CHECKPOINT_CHUNK: &str = \"o3-live-checkpoint\";";
    let wrong_chunk = live.replacen("\"o3-live-checkpoint\"", "\"o3-live-state\"", 1);
    assert_ne!(wrong_chunk, live, "O3LC chunk-name mutation must apply");
    assert!(!wire_contract(
        &wrong_chunk,
        &codec,
        &runtime,
        &pipeline,
        &handoff,
    ));
    let test_only_chunk = live.replacen(
        chunk_declaration,
        &format!("#[cfg(test)]\n{chunk_declaration}"),
        1,
    );
    assert_ne!(test_only_chunk, live, "O3LC cfg(test) mutation must apply");
    assert!(!wire_contract(
        &test_only_chunk,
        &codec,
        &runtime,
        &pipeline,
        &handoff,
    ));
}

#[test]
fn live_checkpoint_cpu_capture_takes_each_state_lock_once() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = fs::read_to_string(crate_dir.join("src/riscv_live_checkpoint.rs")).unwrap();

    assert!(capture_lock_contract(&source));
    for (from, to) in [
        ("self.core.state.lock()", "self.core.state.try_lock()"),
        ("self.state.lock()", "self.state.try_lock()"),
        (
            "let mut projected_state = riscv_state.clone();",
            "let mut projected_state = riscv_state;",
        ),
        (
            "projected_state.finalize_quiescent_o3_writeback_state_for_checkpoint();",
            "self.finalize_quiescent_o3_writeback_for_checkpoint();",
        ),
    ] {
        let mutated = source.replacen(from, to, 1);
        assert_ne!(mutated, source, "capture lock mutation must apply");
        assert!(!capture_lock_contract(&mutated));
    }
    let helper_relock = source.replacen(
        "fn stable_projection(state: &RiscvCoreState) -> crate::O3RuntimeCheckpointPayload {",
        "fn stable_projection(state: &RiscvCoreState) -> crate::O3RuntimeCheckpointPayload {\n    let _relocked = state.lock();",
        1,
    );
    assert_ne!(helper_relock, source, "helper relock mutation must apply");
    assert!(!capture_lock_contract(&helper_relock));
    let added_helper = source.replacen(
        "    let mut stable = stable_projection(state);",
        "    capture_relock(core);\n    let mut stable = stable_projection(state);",
        1,
    )
        + "\nfn capture_relock(core: &RiscvCore) { let _relocked = core.state.lock(); }\n";
    assert_ne!(added_helper, source, "added helper mutation must apply");
    assert!(!capture_lock_contract(&added_helper));
    let added_method = source
        .replacen(
            "    let mut stable = stable_projection(state);",
            "    core.capture_relock();\n    let mut stable = stable_projection(state);",
            1,
        )
        + "\nstruct Decoy;\nimpl Decoy { fn capture_relock(&self) {} }\nimpl RiscvCore { fn capture_relock(&self) { let _relocked = self.state.lock(); } }\n";
    assert_ne!(added_method, source, "added method mutation must apply");
    assert!(!capture_lock_contract(&added_method));

    let capture_decoy = rust_function_definition(&source, "capture_checkpoint_projection").unwrap();
    let guarded_decoy =
        rust_function_definition(&source, "capture_checkpoint_projection_from_guards").unwrap();
    let renamed = source
        .replacen(
            "pub fn capture_checkpoint_projection(",
            "pub fn renamed_checkpoint_projection(",
            1,
        )
        .replacen(
            "fn capture_checkpoint_projection_from_guards(",
            "fn renamed_checkpoint_projection_from_guards(",
            1,
        );
    let disabled_decoy = format!(
        "#[cfg(any())]\nimpl RiscvCore {{ {capture_decoy} }}\n#[cfg(any())]\n{guarded_decoy}\n{renamed}"
    );
    assert!(!capture_lock_contract(&disabled_decoy));
}

#[test]
fn live_checkpoint_cpu_restore_is_prepared_before_installation() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let source =
        fs::read_to_string(crate_dir.join("src/riscv_core_checkpoint_restore.rs")).unwrap();

    assert!(prepared_restore_contract(&source));
    let merged = source.replacen(
        "let detached = RiscvCore",
        "let detached_restore_was_removed = RiscvCore",
        1,
    );
    assert_ne!(merged, source, "detached preparation mutation must apply");
    assert!(!prepared_restore_contract(&merged));

    let prepare_decoy = rust_function_definition(&source, "prepare_checkpoint_restore").unwrap();
    let install_decoy =
        rust_function_definition(&source, "install_prepared_checkpoint_restore").unwrap();
    let renamed = source
        .replacen(
            "pub fn prepare_checkpoint_restore(",
            "pub fn renamed_checkpoint_restore(",
            1,
        )
        .replacen(
            "pub fn install_prepared_checkpoint_restore(",
            "pub fn renamed_install_checkpoint_restore(",
            1,
        );
    let disabled_decoy =
        format!("#[cfg(any())]\nimpl RiscvCore {{ {prepare_decoy} {install_decoy} }}\n{renamed}");
    assert!(!prepared_restore_contract(&disabled_decoy));
}

#[test]
fn live_checkpoint_cpu_ledger_claim_is_bounded_and_score_neutral() {
    let ledger_path = workspace_root().join(LEDGER);
    let ledger = fs::read_to_string(&ledger_path).unwrap();

    assert_eq!(line_count(&ledger_path), 1_200);
    assert!(
        cpu_ledger_contract(&ledger),
        "CPU ledger must retain the 74% score while naming the bounded compute-IQ/one-result authority and all retained gaps"
    );

    let weakened = ledger.replacen("exactly one response-admitted", "response-admitted", 1);
    assert_ne!(weakened, ledger, "bounded-result mutation must apply");
    assert!(!cpu_ledger_contract(&weakened));
}

fn wire_contract(live: &str, codec: &str, runtime: &str, pipeline: &str, handoff: &str) -> bool {
    let active_live = compact_rust_code(&production_rust_source(live));
    let active_codec = compact_rust_code(&production_rust_source(codec));
    let active_runtime = compact_rust_code(&production_rust_source(runtime));
    let active_pipeline = compact_rust_code(&production_rust_source(pipeline));
    let active_handoff = compact_rust_code(&production_rust_source(handoff));
    active_live.contains("pubconstRISCV_O3_LIVE_CHECKPOINT_CHUNK:&str=;")
        && live
            .matches("\"o3-live-checkpoint\"")
            .count()
            == 1
        && active_codec.contains("constMAGIC:[u8;4]=*b;")
        && codec.matches("*b\"O3LC\"").count() == 1
        && active_codec.contains("constVERSION_LEGACY:u8=1;")
        && active_codec.contains("constVERSION_CURRENT:u8=2;")
        && active_runtime
            .contains("constO3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS:u8=23;")
        && active_runtime.contains("constO3_RUNTIME_CHECKPOINT_VERSION:u8=O3_RUNTIME_CHECKPOINT_VERSION_WITH_WRITEBACK_PORT_STATS;")
        && active_pipeline.contains("constO3_PENDING_STATE_CHECKPOINT_VERSION:u8=2;")
        && active_handoff.contains("pub(super)constVERSION_CURRENT:u8=7;")
        && !active_codec.contains("riscv_execution_mode_handoff")
        && !active_codec.contains("RISCV_O3_LIVE_DATA_HANDOFF_CHUNK")
        && !active_codec.contains("RiscvO3LiveDataHandoff")
        && !active_codec.contains("constVERSION:u8=")
        && !codec.contains("*b\"O3DH\"")
}

fn capture_lock_contract(source: &str) -> bool {
    let Some(capture) =
        unconditional_rust_function_definition(source, "capture_checkpoint_projection")
    else {
        return false;
    };
    let Some(guarded) =
        unconditional_rust_function_definition(source, "capture_checkpoint_projection_from_guards")
    else {
        return false;
    };
    let capture = compact_rust_code(&capture);
    let guarded = compact_rust_code(&guarded);
    capture.matches("self.core.state.lock()").count() == 1
        && capture.matches("self.state.lock()").count() == 1
        && capture.contains("letmutprojected_state=riscv_state.clone();")
        && capture
            .contains("projected_state.finalize_quiescent_o3_writeback_state_for_checkpoint();")
        && capture.contains("&projected_state")
        && capture
            .matches("capture_checkpoint_projection_from_guards(")
            .count()
            == 1
        && !guarded.contains(".lock()")
        && transitive_local_helpers_are_lock_free(
            source,
            "capture_checkpoint_projection_from_guards",
        )
}

fn transitive_local_helpers_are_lock_free(source: &str, root: &str) -> bool {
    let mut pending = vec![root.to_string()];
    let mut visited = std::collections::BTreeSet::new();
    while let Some(name) = pending.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let definitions = unconditional_rust_function_definitions(source, &name);
        if definitions.is_empty() {
            return false;
        }
        for definition in definitions {
            if compact_rust_code(&definition).contains(".lock()") {
                return false;
            }
            for called in reachable_function_calls(&definition) {
                if !visited.contains(&called)
                    && !unconditional_rust_function_definitions(source, &called).is_empty()
                {
                    pending.push(called);
                }
            }
        }
    }
    true
}

fn reachable_function_calls(definition: &str) -> Vec<String> {
    let code = rust_code_without_comments_and_literals(definition);
    let body = code.split_once('{').map(|(_, body)| body).unwrap_or("");
    let chars = body.chars().collect::<Vec<_>>();
    let mut calls = Vec::new();
    let mut index = 0;
    while index < chars.len() {
        let Some((identifier, end)) = rust_identifier_at(&chars, index) else {
            index += 1;
            continue;
        };
        let next = skip_rust_whitespace(&chars, end);
        if chars.get(next) == Some(&'(') {
            calls.push(identifier);
        }
        index = end;
    }
    calls
}

fn prepared_restore_contract(source: &str) -> bool {
    let Some(prepare) =
        unconditional_rust_function_definition(source, "prepare_checkpoint_restore")
    else {
        return false;
    };
    let Some(install) =
        unconditional_rust_function_definition(source, "install_prepared_checkpoint_restore")
    else {
        return false;
    };
    let prepare = compact_rust_code(&prepare);
    let install = compact_rust_code(&install);
    prepare.contains("letdetached=RiscvCore{")
        && prepare.contains("prepare_stable(&detached,&input)?;")
        && prepare.contains("prepare_and_install_live(&detached,input.o3.clone(),live)?;")
        && !prepare.contains("self.install_prepared_checkpoint_restore(")
        && install.contains("CpuCore::install_checkpoint_state_into_guard(")
        && install.contains("*riscv_state=prepared.riscv;")
}

fn cpu_ledger_contract(ledger: &str) -> bool {
    let Some(cpu) = ledger_component(ledger, CPU_HEADING) else {
        return false;
    };
    let normalized = cpu.split_whitespace().collect::<Vec<_>>().join(" ");
    normalized.contains(CPU_SCORE)
        && normalized.contains(BOUNDED_CLAIM)
        && normalized.contains(RETAINED_GAPS)
        && !normalized.to_ascii_lowercase().contains("o3lc reuses o3dh")
        && !normalized
            .to_ascii_lowercase()
            .contains("checkpoint-restorable pre-response transport")
}

fn ledger_component<'a>(ledger: &'a str, heading: &str) -> Option<&'a str> {
    let body = ledger.split_once(heading)?.1;
    Some(body.split("\n### ").next().unwrap_or(body))
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
}
