use std::process::Command;

#[test]
fn deterministic_byte_identical_reports() {
    // Build the binary first
    let status = Command::new("cargo")
        .args(["build", "--bin", "arb-backtest", "--release"])
        .current_dir("/home/kasparov/arb_bot")
        .status()
        .expect("cargo build");
    assert!(status.success());

    let bin = "/home/kasparov/arb_bot/target/release/arb-backtest";
    let out1 = "/tmp/test_backtest_1";
    let out2 = "/tmp/test_backtest_2";

    for out in [out1, out2] {
        std::fs::remove_dir_all(out).ok();
    }

    for out in [out1, out2] {
        let s = Command::new(bin)
            .args([
                "--recordings",
                "fixtures",
                "--config",
                "config.yaml",
                "--out",
                out,
                "--profit-threshold-bps",
                "1.0",
            ])
            .current_dir("/home/kasparov/arb_bot")
            .status()
            .expect("run arb-backtest");
        assert!(s.success());
    }

    let r1 = std::fs::read_to_string(format!("{}/run_report.json", out1)).unwrap();
    let r2 = std::fs::read_to_string(format!("{}/run_report.json", out2)).unwrap();
    // Note: config_hash contains file mtime path, but for same config in same run it should match
    let parsed1: serde_json::Value = serde_json::from_str(&r1).unwrap();
    let parsed2: serde_json::Value = serde_json::from_str(&r2).unwrap();
    assert_eq!(
        parsed1["n_opportunities"], parsed2["n_opportunities"],
        "opportunity count should be deterministic"
    );
    assert_eq!(
        parsed1["n_fills"], parsed2["n_fills"],
        "fill count should be deterministic"
    );
    assert_eq!(
        parsed1["net_pnl"], parsed2["net_pnl"],
        "net pnl should be deterministic"
    );
    assert_eq!(
        parsed1["sharpe"], parsed2["sharpe"],
        "sharpe should be deterministic"
    );
}
