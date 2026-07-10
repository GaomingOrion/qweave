use std::path::PathBuf;

use clap::Parser;
use qweave_server::{ReportData, run_server};

/// Serve an interactive factor-evaluation report from a saved output dir.
#[derive(Parser)]
#[command(name = "qweave-server", version)]
struct Args {
    /// Evaluation output directory (from `evaluate(output_dir=...)` / `save()`).
    #[arg(long)]
    dir: PathBuf,
    /// Port to listen on (0 picks a free port).
    #[arg(long, default_value_t = 8080)]
    port: u16,
    /// Serve this `dist` dir instead of the embedded frontend (dev use).
    #[arg(long)]
    assets: Option<PathBuf>,
    /// Open the report in the default browser once the server is up.
    #[arg(long)]
    open: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    let data = ReportData::from_dir(&args.dir)?;
    run_server(data, args.port, args.open, args.assets)?;
    Ok(())
}
