use crate::sweep::SweepResult;

pub fn write_csv(path: &str, rows: &[SweepResult]) -> anyhow::Result<()> {
    let mut wtr = csv::Writer::from_path(path)?;
    wtr.write_record([
        "profit_threshold_bps",
        "notional_usd",
        "usd_usdt_basis_bps",
        "slippage_alpha",
        "strategies",
        "num_opps",
        "avg_net_bps",
        "sharpe",
        "max_drawdown_bps",
        "win_rate",
    ])?;
    for r in rows {
        wtr.write_record([
            format!("{:.2}", r.combo.profit_threshold_bps),
            format!("{:.2}", r.combo.notional_usd),
            format!("{:.2}", r.combo.usd_usdt_basis_bps),
            format!("{:.4}", r.combo.slippage_alpha),
            r.combo.strategies.join(","),
            r.num_opps.to_string(),
            format!("{:.4}", r.avg_net_bps),
            format!("{:.4}", r.sharpe),
            format!("{:.4}", r.max_drawdown_bps),
            format!("{:.4}", r.win_rate),
        ])?;
    }
    wtr.flush()?;
    Ok(())
}
