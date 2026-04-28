use arb_backtest::metrics::RunReport;
use arb_backtest::replay::replay_from_dir;
use arb_backtest::simulator::run_simulation;
use arb_config::loader::load as load_config;
use arb_core::types::BuildMode;
use clap::Parser;
use std::path::Path;
use tracing::info;

#[derive(Debug, Parser)]
#[command(name = "arb-backtest")]
struct Args {
    #[arg(long, default_value = "recordings/latest")]
    recordings: String,
    #[arg(long, default_value = "config.yaml")]
    config: String,
    #[arg(long, default_value = "run_artifacts/backtest")]
    out: String,
    #[arg(long, default_value = "1.0")]
    speed: f64,
    #[arg(long)]
    profit_threshold_bps: Option<f64>,
    #[arg(long)]
    notional_usd: Option<f64>,
    #[arg(long, default_value = "0.05")]
    slippage_alpha: f64,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("info"))
        .init();

    let args = Args::parse();
    let cfg = load_config(Path::new(&args.config))?;
    info!("Loaded config mode={:?}", cfg.mode);

    let symbol_maps = cfg.symbol_maps();
    let enabled = cfg.enabled_venues();

    let engine_cfg = arb_backtest::engine_config_from(
        &cfg,
        symbol_maps,
        enabled,
        args.slippage_alpha,
        args.profit_threshold_bps,
        args.notional_usd,
    );
    // Freeze mode to Paper for replay determinism
    let engine_cfg = {
        let mut c = engine_cfg;
        c.mode = BuildMode::Paper;
        c
    };

    let updates = replay_from_dir(&args.recordings)?;
    info!("Loaded {} updates", updates.len());

    let out_dir = std::path::PathBuf::from(&args.out);
    std::fs::create_dir_all(&out_dir)?;
    let ledger = out_dir.join("paper_ledger.jsonl");

    let sim_result = run_simulation(updates.clone(), engine_cfg, &ledger, args.speed).await?;
    info!(
        "Simulation complete: {} opportunities, {} fills",
        sim_result.opportunities.len(),
        sim_result.records.len()
    );

    let config_hash = format!("{:08x}", {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut s = DefaultHasher::new();
        std::fs::read_to_string(&args.config)?.hash(&mut s);
        args.profit_threshold_bps.map(|v| format!("{:.6}", v)).hash(&mut s);
        args.notional_usd.map(|v| format!("{:.6}", v)).hash(&mut s);
        format!("{:.6}", args.slippage_alpha).hash(&mut s);
        s.finish()
    });

    let report = RunReport::compute(
        &sim_result.records,
        env!("CARGO_PKG_VERSION"),
        &config_hash,
        updates.len(),
    );

    let report_path = out_dir.join("run_report.json");
    std::fs::write(&report_path, serde_json::to_string_pretty(&report)?)?;
    info!("Report written to {}", report_path.display());

    Ok(())
}
