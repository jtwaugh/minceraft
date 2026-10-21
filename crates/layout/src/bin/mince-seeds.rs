//! `mince-seeds --levels N --candidates N --top N --out PATH [--start SEED]`:
//! search seeds and write the best layout as a level file.
use std::process::ExitCode;

use layout::{render, search_seeds, Args, ScoringConfig, SearchProgress, Thresholds, USAGE};

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let found = search_seeds(
        &args.search_params(),
        &Thresholds::shipped(),
        &ScoringConfig::shipped(),
        &SearchProgress::default(),
    );
    let Some(best) = found.first() else {
        eprintln!("error: no candidate seed passed validation; nothing written");
        return ExitCode::FAILURE;
    };
    for (i, s) in found.iter().enumerate() {
        println!("{:>3}. seed {} score {:.4}", i + 1, s.seed, s.score);
    }
    let text = match render(best) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };
    if let Err(e) = std::fs::write(&args.out, text) {
        eprintln!("error: {}: {e}", args.out);
        return ExitCode::FAILURE;
    }
    println!("wrote {}", args.out);
    ExitCode::SUCCESS
}
