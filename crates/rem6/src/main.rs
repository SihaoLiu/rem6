fn main() {
    match rem6::run_cli(std::env::args().skip(1)) {
        Ok(output) => {
            print!("{output}");
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}
