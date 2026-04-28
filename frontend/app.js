/* ========================================================
   Arb Bot Dashboard v2 — Backtest + Live Analytics
   ======================================================== */
const $ = id => document.getElementById(id);
const fmt = n => (n == null ? '-' : (typeof n === 'number' ? n.toFixed(4) : n));
const fmtCur = n => (n == null ? '-' : '$' + n.toFixed(2));
const fmtPct = n => (n == null ? '-' : (n * 100).toFixed(2) + '%');
const fmtBps = n => (n == null ? '-' : n.toFixed(2) + ' bps');
const sign = v => v >= 0 ? 'positive' : 'negative';
const fmtTs = ts => {
  if (!ts) return '-';
  const s = String(ts).length > 13 ? Math.floor(Number(ts) / 1e6) : Number(ts);
  return new Date(s).toISOString().replace('T',' ').slice(0,19);
};

let pnlChart = null, feeChart = null, tradeHistChart = null, sweepChart = null;
let currentFiles = {};

/* ---------- Core Report Loading ---------- */
function loadReport(text) {
  const r = JSON.parse(text);
  currentFiles['run_report.json'] = text; // store for simulator
  $('netPnl').textContent = fmtCur(r.net_pnl);
  $('netPnl').className = 'value ' + sign(r.net_pnl);
  $('grossPnl').textContent = fmtCur(r.gross_pnl);
  $('grossPnl').className = 'value ' + sign(r.gross_pnl);
  $('opps').textContent = r.n_opportunities ?? '-';
  $('fills').textContent = r.n_fills ?? '-';
  $('fillRate').textContent = fmtPct(r.fill_rate_pct);
  $('sharpe').textContent = fmt(r.sharpe);
  $('drawdown').textContent = fmtPct(r.max_drawdown_pct);
  $('slippage').textContent = fmtBps(r.realized_slippage_bps);
  const totalFees = r.gross_pnl - r.net_pnl;
  $('totalFees').textContent = fmtCur(totalFees);
  $('avgNetPerTrade').textContent = fmtCur(r.n_fills ? r.net_pnl / r.n_fills : 0);

  /* per-symbol table */
  const symBody = $('symbolTable').querySelector('tbody');
  symBody.innerHTML = '';
  const syms = r.per_symbol || {};
  for (const [k, v] of Object.entries(syms)) {
    const tr = document.createElement('tr');
    const sClass = (v.net_pnl || 0) >= 0 ? 'positive' : 'negative';
    tr.innerHTML = `<td>${k}</td><td>${v.opp_count}</td><td>${v.fills}</td><td>${fmtCur(v.gross_pnl)}</td><td class="${sClass}">${fmtCur(v.net_pnl)}</td>`;
    symBody.appendChild(tr);
  }

  /* capital simulator trigger */ updateCapital();

  return r;
}

function loadLedger(text) {
  const lines = text.trim().split(/\r?\n/).filter(Boolean);
  const rows = lines.map(l => { try { return JSON.parse(l); } catch(_) { return null; } }).filter(Boolean);
  const tbody = $('tradesTable').querySelector('tbody');
  tbody.innerHTML = '';
  let cum = 0, labels = [], data = [], bins = new Array(10).fill(0), minPnL = Infinity, maxPnL = -Infinity;
  const histBins = [-0.5, 2.5, 5.0, 10.0, 20.0, 50.0, 100.0];
  let histData = new Array(histBins.length - 1).fill(0);

  rows.forEach((r, i) => {
    const pnl = r.sim_pnl || 0;
    cum += pnl;
    labels.push(i + 1);
    data.push(cum);
    minPnL = Math.min(minPnL, pnl);
    maxPnL = Math.max(maxPnL, pnl);
    for (let b = 0; b < histBins.length - 1; b++) {
      if (pnl >= histBins[b] && pnl < histBins[b+1]) { histData[b]++; break; }
    }
    const tr = document.createElement('tr');
    const buy = r.buy_price * r.buy_qty;
    const sell = r.sell_price * r.sell_qty;
    const totalFee = (r.fee_buy || 0) + (r.fee_sell || 0);
    const pnlClass = pnl >= 0 ? 'positive' : 'negative';
    tr.innerHTML = `
      <td>${fmtTs(r.ts_ns)}</td>
      <td>${r.symbol}</td>
      <td>${r.buy_venue}</td>
      <td>${r.sell_venue}</td>
      <td>${fmtCur(buy)}</td>
      <td>${fmtCur(sell)}</td>
      <td>${fmt(r.buy_qty)}</td>
      <td>${fmtCur(totalFee)}</td>
      <td class="${pnlClass}">${fmtCur(pnl)}</td>`;
    tbody.appendChild(tr);
  });

  /* stats on rows */
  const wins = rows.filter(r => (r.sim_pnl||0) > 0).length;
  const winRate = rows.length ? (wins / rows.length) : 0;
  $('tradeCount').textContent = rows.length;
  $('winRate').textContent = fmtPct(winRate);
  $('minTrade').textContent = fmtCur(minPnL === Infinity ? 0 : minPnL);
  $('maxTrade').textContent = fmtCur(maxPnL === -Infinity ? 0 : maxPnL);
  $('maxTrade').className = 'value ' + sign(maxPnL);

  renderPnlChart(labels, data);
  renderFeeChart(rows);
  renderTradeHist(histBins, histData);
  $('summary').classList.remove('hidden');
}

/* ---------- Charts ---------- */
function renderPnlChart(labels, data) {
  const ctx = $('pnlChart').getContext('2d');
  if (pnlChart) pnlChart.destroy();
  pnlChart = new Chart(ctx, {
    type: 'line',
    data: {
      labels: labels,
      datasets: [{
        label: 'Cumulative PnL ($)',
        data: data,
        borderColor: '#3fb950',
        backgroundColor: 'rgba(63,185,80,0.08)',
        fill: true,
        tension: 0.3,
        pointRadius: 1.5,
        pointHoverRadius: 4,
        borderWidth: 2
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      interaction: { mode: 'index', intersect: false },
      plugins: {
        legend: { labels: { color: '#c9d1d9' } },
        tooltip: { backgroundColor: '#161b22', titleColor: '#c9d1d9', bodyColor: '#c9d1d9', borderColor: '#30363d', borderWidth: 1 }
      },
      scales: {
        x: { ticks: { color: '#8b949e', maxTicksLimit: 20 }, grid: { color: '#21262d' } },
        y: { ticks: { color: '#8b949e', callback: v => '$' + v.toFixed(2) }, grid: { color: '#21262d' } }
      }
    }
  });
}

function renderFeeChart(rows) {
  let totalGross = 0, totalFees = 0;
  rows.forEach(r => {
    const pnl = r.sim_pnl || 0;
    const fees = (r.fee_buy || 0) + (r.fee_sell || 0);
    totalGross += pnl + fees;
    totalFees += fees;
  });
  const net = totalGross - totalFees;
  const ctx = $('feeChart').getContext('2d');
  if (feeChart) feeChart.destroy();
  feeChart = new Chart(ctx, {
    type: 'doughnut',
    data: {
      labels: ['Net Profit', 'Exchange Fees + Slippage'],
      datasets: [{
        data: [Math.max(0, net), totalFees],
        backgroundColor: ['#3fb950', '#f85149'],
        borderColor: '#0b0e14',
        borderWidth: 3
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { position: 'bottom', labels: { color: '#c9d1d9', padding: 20 } },
        tooltip: { callbacks: { label: c => ' ' + c.label + ': ' + fmtCur(c.raw) } }
      }
    }
  });
}

function renderTradeHist(bins, data) {
  const ctx = $('tradeHistChart').getContext('2d');
  if (tradeHistChart) tradeHistChart.destroy();
  const labels = bins.slice(0, -1).map((b, i) => `$${b}–${bins[i+1]} (${data[i]})`);
  tradeHistChart = new Chart(ctx, {
    type: 'bar',
    data: {
      labels: labels,
      datasets: [{
        label: 'Trades',
        data: data,
        backgroundColor: '#58a6ff',
        borderRadius: 4
      }]
    },
    options: {
      responsive: true,
      maintainAspectRatio: false,
      plugins: {
        legend: { display: false },
        tooltip: { backgroundColor: '#161b22', titleColor: '#c9d1d9', bodyColor: '#c9d1d9', borderColor: '#30363d', borderWidth: 1 }
      },
      scales: {
        x: { ticks: { color: '#8b949e', maxRotation: 45 }, grid: { display: false } },
        y: { ticks: { color: '#8b949e', stepSize: 1 }, grid: { color: '#21262d' } }
      }
    }
  });
}

/* ---------- Capital Simulator ---------- */
function updateCapital() {
  const cap = parseFloat($('capInput').value) || 1000;
  const feeBps = parseFloat($('feeInput').value) || 26;
  const trades = parseInt($('tradeInput').value) || 48;

  // Compute from current report if available
  const reportText = currentFiles['run_report.json'];
  let scalar = 1;
  if (reportText) {
    try {
      const r = JSON.parse(reportText);
      const baseNotional = r.per_symbol ? Object.values(r.per_symbol)[0].fills ? 1000 : 1000 : 1000;
      // Approx linear scaling
      if (r.n_fills) {
        const baseCap = 1000;
        const avgNet = r.net_pnl / r.n_fills;
        const avgGross = r.gross_pnl / r.n_fills;
        const avgFee = avgGross - avgNet;
        const scale = (cap / baseCap) * (trades / r.n_fills);

        const projectedGross = avgGross * r.n_fills * scale;
        const projectedFees = avgFee * r.n_fills * scale;
        const projectedNet = avgNet * r.n_fills * scale;
        $('projGross').textContent = fmtCur(projectedGross);
        $('projFees').textContent = fmtCur(projectedFees);
        $('projNet').textContent = fmtCur(projectedNet);
        $('projNet').className = 'value ' + sign(projectedNet);
        $('projPerTrade').textContent = fmtCur(projectedNet / trades);
        return;
      }
    } catch(_) {}
  }

  // Fallback simple model
  const projGross = cap * (15 / 10000) * trades; // assume 15 bps edge per trade
  const projFees = cap * (feeBps / 10000) * trades;
  const projNet = projGross - projFees;
  $('projGross').textContent = fmtCur(projGross);
  $('projFees').textContent = fmtCur(projFees);
  $('projNet').textContent = fmtCur(projNet);
  $('projNet').className = 'value ' + sign(projNet);
  $('projPerTrade').textContent = fmtCur(projNet / trades);
}

['capInput', 'feeInput', 'tradeInput'].forEach(id => {
  $(id).addEventListener('input', updateCapital);
});

/* ---------- File Handling ---------- */
async function readFile(f) { return new Promise((res, rej) => { const r = new FileReader(); r.onload = e => res(e.target.result); r.onerror = rej; r.readAsText(f); }); }

async function handleFiles(files) {
  let reportText, ledgerText;
  const arr = Array.from(files);
  for (const f of arr) {
    const text = await readFile(f);
    if (f.name.endsWith('run_report.json')) { reportText = text; currentFiles['run_report.json'] = text; }
    if (f.name.endsWith('paper_ledger.jsonl')) { ledgerText = text; currentFiles['paper_ledger.jsonl'] = text; }
  }
  if (reportText) {
    try { loadReport(reportText); } catch(e) { alert('Failed to parse run_report.json: ' + e); }
  }
  if (ledgerText) {
    try { loadLedger(ledgerText); } catch(e) { alert('Failed to parse paper_ledger.jsonl: ' + e); }
  }
}

const dz = $('dropzone');
const fi = $('fileInput');
dz.addEventListener('click', () => fi.click());
dz.addEventListener('dragover', e => { e.preventDefault(); dz.classList.add('dragover'); });
dz.addEventListener('dragleave', () => dz.classList.remove('dragover'));
dz.addEventListener('drop', async e => {
  e.preventDefault(); dz.classList.remove('dragover');
  const files = Array.from(e.dataTransfer.files);
  await handleFiles(files);
});
fi.addEventListener('change', async e => { await handleFiles(Array.from(e.target.files)); });

/* ---------- Auto-load nearby ---------- */
(async () => {
  const candidates = [
    '../run_artifacts/latest_backtest/run_report.json',
    '../run_artifacts/latest_backtest_1bps/run_report.json',
    '../run_artifacts/rd_1000_0.5/run_report.json',
    './sample_data/run_report.json',
    './sample_data/paper_ledger.jsonl'
  ];
  for (const path of candidates) {
    try {
      const r = await fetch(path).then(r => r.ok ? r.text() : null).catch(() => null);
      if (r) { loadReport(r); break; }
    } catch(_) {}
  }
  for (const path of candidates.map(p => p.replace('run_report.json', 'paper_ledger.jsonl'))) {
    try {
      const l = await fetch(path).then(r => r.ok ? r.text() : null).catch(() => null);
      if (l) { loadLedger(l); break; }
    } catch(_) {}
  }
})();
