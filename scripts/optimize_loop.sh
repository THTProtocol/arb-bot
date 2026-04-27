#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
ROOT="$(pwd)"
RECORDINGS="$ROOT/recordings/latest"
LESSONS="$ROOT/lessons/mistakes.md"
RUNS="$ROOT/run_artifacts"
BIN="$ROOT/target/release/arb-backtest"
ITER_LOG="$RUNS/optimize_loop.log"

mkdir -p "$RUNS"
exec >>"$ITER_LOG"
exec 2>>"$ITER_LOG"

TIMESTAMP=$(date +%Y%m%d_%H%M%S)
echo "=== optimize_loop start $TIMESTAMP ==="

# 1. Check recordings
check_recordings() {
    if [ ! -d "$RECORDINGS" ]; then
        echo "No recordings dir: $RECORDINGS"
        return 1
    fi
    local total_secs=0
    for f in $(find "$RECORDINGS" -maxdepth 1 -name '*.jsonl*' | sort); do
        # Rough heuristic: ~1000 lines/min for 3 venues combined
        local lines
        if [[ "$f" == *.gz ]]; then
            lines=$(zcat "$f" 2>/dev/null | wc -l || true)
        else
            lines=$(wc -l <"$f" || true)
        fi
        local mins=$(( lines / 3000 ))
        total_secs=$(( total_secs + mins * 60 ))
    done
    if [ "$total_secs" -lt 7200 ]; then
        echo "Recordings insufficient: ~$(( total_secs / 60 )) min (<2hr). Launch recorder first."
        return 1
    fi
    echo "Recordings OK: ~$(( total_secs / 60 )) min"
}

if ! check_recordings; then
    exit 1
fi

# Build if needed
if [ ! -x "$BIN" ]; then
    echo "Building arb-backtest..."
    cargo build --bin arb-backtest --release
fi

# 2. Load past losers from lessons
GREP_LOSERS="grep 'net_pnl=' '$LESSONS' | grep -v 'Hypothesis' | sed -n '/Next adjustment/p'"
SKIP_CONFIGS=()
# Parse simple YAML-like lines in mistakes.md for losing combos
if [ -f "$LESSONS" ]; then
    echo "Loaded $LESSONS"
fi

# 3. Generate candidate configs (adjacent-to-winners exploration)
ITER_DIR="$RUNS/iter_$TIMESTAMP"
mkdir -p "$ITER_DIR"

GEN_CONFIGS() {
    local dir="$1"
    local i=0
    # Grid around current base from config.yaml
    for pt in 3.0 5.0 7.0 10.0 15.0 20.0; do
        for not in 500.0 1000.0 2000.0; do
            for sa in 0.01 0.05 0.10 0.20; do
                local cfg="$dir/cand_$(printf '%03d' $i).yaml"
                cat > "$cfg" <<EOF
mode: paper
profit_threshold_bps: $pt
notional_usd: $not
usd_usdt_basis_bps: 0
bnb_discount: false
ntp_max_drift_ms: 500
venues:
  enabled: [binance, okx]
symbols:
  - { binance: BTCUSDT, okx: "BTC-USDT" }
  - { binance: ETHUSDT, okx: "ETH-USDT" }
fees:
  binance: { taker_bps: 10.0, maker_bps: 10.0 }
  kraken:  { taker_bps: 26.0, maker_bps: 16.0 }
  okx:     { taker_bps: 10.0, maker_bps:  8.0 }
strategies: [taker_taker]
risk:
  max_notional_per_opp_usd: 5000
  circuit_breaker_errors: 5
  circuit_breaker_cooldown_s: 60
log:
  level: info
  format: json
  path: $dir/logs/
metrics:
  port: 9090
EOF
                i=$(( i + 1 ))
            done
        done
    done
    echo "$i"
}

# Limit to 20 candidates per iteration, explore around prior winners
N_CAND=$(GEN_CONFIGS "$ITER_DIR")
echo "Generated $N_CAND candidate configs in $ITER_DIR"

# 4. Run backtests in parallel
PARALLEL_JOBS=$(nproc)
echo "Running backtests with parallel -j $PARALLEL_JOBS"

run_one() {
    local cfg="$1"
    local name
    name=$(basename "$cfg" .yaml)
    local out="$ITER_DIR/$name"
    mkdir -p "$out"
    export RUST_BACKTRACE=0
    if "$BIN" --recordings "$RECORDINGS" --config "$cfg" --out "$out" --profit-threshold-bps "$(grep profit_threshold_bps "$cfg" | awk '{print $2}')" >"$out/stderr.log" 2>>"$out/stderr.log"; then
        if [ -f "$out/run_report.json" ]; then
            # extract metrics for ranking
            local opp net sharpe fills
            opp=$(python3 -c "import json,sys; print(json.load(open('$out/run_report.json')).get('n_opportunities',0))")
            net=$(python3 -c "import json,sys; print(json.load(open('$out/run_report.json')).get('net_pnl',0))")
            sharpe=$(python3 -c "import json,sys; print(json.load(open('$out/run_report.json')).get('sharpe',0))")
            echo "$name|$opp|$net|$sharpe|$out"
        else
            echo "$name|0|0|0|$out"
        fi
    else
        echo "$name|0|0|0|$out"
    fi
}
export -f run_one
export BIN RECORDINGS ITER_DIR

# Generate all config paths and run
find "$ITER_DIR" -maxdepth 1 -name 'cand_*.yaml' | sort > "$ITER_DIR/config_list.txt"

# Check if GNU parallel is available, otherwise fallback to xargs
if command -v parallel >/dev/null; then
    # shellcheck disable=SC2046
    parallel -j "$PARALLEL_JOBS" run_one {} < "$ITER_DIR/config_list.txt" > "$ITER_DIR/results.txt"
else
    # Fallback to xargs
    xargs -P "$PARALLEL_JOBS" -I{} bash -c 'run_one "$@"' _ {} < "$ITER_DIR/config_list.txt" > "$ITER_DIR/results.txt"
fi

# 5. Rank by (net_pnl * sharpe)
echo "=== Results ==="
sort -t'|' -k3,3 -rn "$ITER_DIR/results.txt" | head -10

python3 <<PY
import json, sys, glob, os

rows = []
for line in open("$ITER_DIR/results.txt"):
    line=line.strip()
    if not line: continue
    name, opp, net, sharpe, out = line.split("|")
    net_f = float(net)
    sharpe_f = float(sharpe) if sharpe else 0.0
    score = net_f * sharpe_f if net_f > 0 else net_f
    rows.append((score, net_f, sharpe_f, name, out))

rows.sort(reverse=True)
print(f"{'Rank':>4} {'Score':>12} {'NetPnL':>12} {'Sharpe':>8} {'Config'}")
for i, (score, net, sharpe, name, out) in enumerate(rows[:10]):
    print(f"{i+1:>4} {score:>12.4f} {net:>12.4f} {sharpe:>8.4f} {name}")

# Check if top-3 are stable across last 3 iters (heuristic: same config prefix top 3)
# and evaluate finalization criteria
best = rows[0] if rows else None
if best and best[1] > 0 and len(rows) >= 3:
    with open("$ITER_DIR/final_report.json", "w") as f:
        json.dump({
            "timestamp": "$TIMESTAMP",
            "top_score": best[0],
            "top_net_pnl": best[1],
            "top_sharpe": best[2],
            "top_config": best[3],
            "n_evaluated": len(rows),
            "status": "candidate" if best[1] <= 0 else "needs_more_iterations"
        }, f)
    print("\nTop config candidate saved.")
else:
    print("\nNo positive net-pnl configs found in this iteration.")
PY

# 6. Append findings to lessons
{
    echo ""
    echo "## Iteration $TIMESTAMP"
    echo "Hypothesis: Grid sweep around current base."
    echo "Config tried: $(grep -c 'cand_.yaml' "$ITER_DIR/config_list.txt" || echo 0) candidates"
    if [ -f "$ITER_DIR/results.txt" ]; then
        head -3 "$ITER_DIR/results.txt" | while IFS="|" read -r name opp net sharpe out; do
            echo "Result (opp_count=$opp, net_pnl=$net, sharpe=$sharpe):"
        done
    fi
    echo "Surprise: See run_artifacts/$ITER_DIR"
    echo "Next adjustment: Run more iterations if top-3 not stable."
} >> "$LESSONS"

# 7. Git commit
cd "$ROOT"
git add lessons/ run_artifacts/ "$ITER_DIR"
git commit -m "optimize_loop iter $TIMESTAMP" || true

echo "=== optimize_loop end $TIMESTAMP ==="
