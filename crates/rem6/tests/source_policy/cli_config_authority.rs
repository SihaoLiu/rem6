use std::fs;
use std::path::Path;

use super::{line_count, rust_function_definition_names, rust_source_files};

const MAX_CLI_CONFIG_LINES: usize = 500;
const MAX_CONFIG_ROOT_LINES: usize = 1700;

#[test]
fn core_cli_config_mechanics_have_one_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = fs::read_to_string(crate_dir.join("src/lib.rs")).unwrap();
    let config_path = crate_dir.join("src/config.rs");
    let config = fs::read_to_string(&config_path).unwrap();
    let parse_path = crate_dir.join("src/config/parse.rs");
    let parse = fs::read_to_string(&parse_path).unwrap();
    let cli_config_path = crate_dir.join("src/cli_config.rs");

    assert!(
        lib.lines().any(|line| line.trim() == "mod cli_config;"),
        "src/lib.rs must declare the shared CLI config authority"
    );
    assert!(
        cli_config_path.is_file(),
        "core CLI config mechanics belong in src/cli_config.rs"
    );
    assert!(
        !crate_dir.join("src/config/file_scan.rs").exists(),
        "src/config/file_scan.rs must not remain as a second config-scan authority"
    );
    assert!(
        line_count(&cli_config_path) <= MAX_CLI_CONFIG_LINES,
        "src/cli_config.rs must stay focused"
    );
    assert!(
        line_count(&config_path) < MAX_CONFIG_ROOT_LINES,
        "src/config.rs must stay below {MAX_CONFIG_ROOT_LINES} lines"
    );

    let cli_config = fs::read_to_string(&cli_config_path).unwrap();
    let cli_config_functions = rust_function_definition_names(&cli_config);
    for function in [
        "config_path_from_args",
        "run_file_config_from_args",
        "gups_file_config_from_args",
        "trace_replay_file_config_from_args",
        "read_toml_config",
        "required_value",
        "resolve_config_path",
    ] {
        assert!(
            cli_config_functions.contains(function),
            "src/cli_config.rs must own `{function}`"
        );
    }

    let config_functions = rust_function_definition_names(&config);
    assert!(
        !config_functions.contains("resolve_config_path"),
        "src/config.rs must delegate path resolution to src/cli_config.rs"
    );
    for local_error_construction in ["Rem6CliError::ReadConfig", "Rem6CliError::ParseConfig"] {
        assert!(
            !config.contains(local_error_construction),
            "src/config.rs must not locally construct `{local_error_construction}`"
        );
    }

    let parse_functions = rust_function_definition_names(&parse);
    assert!(
        !parse_functions.contains("required_value"),
        "src/config/parse.rs must not define its own required_value helper"
    );
    assert!(
        parse.contains("pub(super) use crate::cli_config::required_value;"),
        "src/config/parse.rs must re-export crate::cli_config::required_value"
    );
}

#[test]
fn auxiliary_commands_consume_cli_config_authority() {
    let crate_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cli_config_path = crate_dir.join("src/cli_config.rs");
    let authority = fs::read_to_string(&cli_config_path).unwrap();

    for wrapper in [
        "gpu_run_file_config_from_args",
        "accelerator_run_file_config_from_args",
        "multi_run_file_config_from_args",
        "resource_acquire_file_config_from_args",
    ] {
        assert!(
            authority.contains(&format!("pub(crate) fn {wrapper}(")),
            "src/cli_config.rs must own `{wrapper}`"
        );
    }

    for relative in [
        "src/gpu_cli.rs",
        "src/accelerator_cli.rs",
        "src/multi_run_cli.rs",
        "src/resource_acquire_config.rs",
    ] {
        let source = fs::read_to_string(crate_dir.join(relative)).unwrap();
        assert!(
            source.contains("crate::cli_config"),
            "{relative} must consume crate::cli_config"
        );
        for local_error_construction in ["Rem6CliError::ReadConfig", "Rem6CliError::ParseConfig"] {
            assert!(
                !source.contains(local_error_construction),
                "{relative} must not locally construct `{local_error_construction}`"
            );
        }
        assert!(
            !source.contains("fn required_value("),
            "{relative} must not define required_value"
        );
        assert!(
            !source.contains("if path.is_relative()"),
            "{relative} must delegate config path resolution"
        );
    }

    assert!(
        !fs::read_to_string(crate_dir.join("src/power_import_cli.rs"))
            .unwrap()
            .contains("fn required_value("),
        "src/power_import_cli.rs must not define required_value"
    );

    for path in rust_source_files(&crate_dir.join("src")) {
        let source = fs::read_to_string(&path).unwrap();
        for function in [
            "config_path_from_args",
            "required_value",
            "resolve_config_path",
        ] {
            if source.contains(&format!("fn {function}(")) {
                assert_eq!(
                    path,
                    cli_config_path,
                    "{} must not define `{function}`",
                    path.strip_prefix(crate_dir).unwrap().display()
                );
            }
        }
    }
}
