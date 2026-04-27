# Arb-Bot Strategy Optimization Mistake Log

## 2025-04-24 12:00 — Baseline
Hypothesis: Taker-taker only works for cross-venue spreads.
Config tried: profit_threshold_bps=15, notional_usd=1000, strategies=[taker_taker]
Result (opp_count=8, fill_rate=?, net_pnl=? USD, sharpe=?):
Surprise: Not enough data yet.
Next adjustment: Collect >2hrs recordings then run grid sweep.

## Iteration N — YYYY-MM-DD HH:MM
Hypothesis: Lower threshold captures more micro-opportunities.
Config tried: profit_threshold_bps=10, notional_usd=500, slippage_alpha=0.05
Result (opp_count=?, fill_rate=?, net_pnl=?, sharpe=?):
Surprise:
Next adjustment:
