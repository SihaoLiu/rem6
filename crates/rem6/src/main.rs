fn main() {
    match rem6::run_cli_with_diagnostics(std::env::args().skip(1)) {
        Ok(output) => {
            print!("{output}");
        }
        Err(error) => {
            eprintln!("{error}");
            if let Some(diagnostic_json) = error.diagnostic_json() {
                eprintln!("{diagnostic_json}");
            }
            std::process::exit(2);
        }
    }
}
