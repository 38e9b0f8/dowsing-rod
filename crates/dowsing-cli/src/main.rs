fn main() {
    if let Err(error) = dowsing_cli::run_cli_from(std::env::args().collect()) {
        eprintln!("error: {error:#}");
        std::process::exit(1);
    }
}
