/* ========================================================
   Arb Bot Dashboard v3 — Backtest + Live Analytics
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

let pnlChart = null, feeChart = null, tradeHistChart = null;
let currentFiles = {};
let _sortCol = null, _sortAsc = true;

/* ---------- Core Report Loading ---------- */
function loadReport(text) {
  const r = JSON.parse(text);
  currentFiles['run_report.json'] = text;
  $('netPnl').textContent = fmtCur(r.net_pnl);
  $('netPnl').className = 'value ' + sign(r.net_pnl);
  $('grossPnl').textContent = fmtCur(r.gross_pnl);
  $('grossPnl').className = 'value ' + sign(r.gross_pnl);
  $('opps').textContent = r.n_opportunities ?? '-';
  $('fills').textContent = r.n_fills ?? '-';
  $('fillRate').textContent = fmtPct(r.fill_rate_pct);

  // Sharpe: green >1, red <0, otherwise white
  const sharpeEl = $('sharpe');
  sharpeEl.textContent = fmt(r.sharpe);
  sharpeEl.className = 'value' + (r.sharpe > 1 ? ' positive' : r.sharpe < 0 ? ' negative' : '');

  // Drawdown: red if >5%
  const ddEl = $('drawdown');
  ddEl.textContent = fmtPct(r.max_drawdown_pct);
  ddEl.className = 'value' + (r.max_drawdown_pct > 0.05 ? ' negative' : '');

  $('slippage').textContent = fmtBps(r.realized_slippage_bps);
  const totalFees = r.gross_pnl - r.net_pnl;
  $('totalFees').textContent = fmtCur(totalFees);
  $('avgNetPerTrade').textContent = fmtCur(r.n_fills ? r.net_pnl / r.n_fills : 0);

  // per-symbol table
  renderSymbolTable(r.per_symbol || {});

  updateCapital();
  return r;
}

// Symbol table with sort support
let _symbolData = {};
function renderSymbolTable(syms, sortCol, asc) {
  _symbolData = syms;
  const entries = Object.entries(syms);
  if (sortCol) {
    const colMap = { 'opp_count': 0, 'fills': 1, 'gross_pnl': 2, 'net_pnl': 3 };
    entries.sort((a, b) => {
      const va = a[1][sortCol] || 0, vb = b[1][sortCol] || 0;
      return asc ? va - vb : vb - va;
    });
  }
  const symBody = $('symbolTable').querySelector('tbody');
  symBody.innerHTML = '';
  for (const [k, v] of entries) {
    const tr = document.createElement('tr');
    const sClass = (v.net_pnl || 0) >= 0 ? 'positive' : 'negative';
    tr.innerHTML = `<td>${k}</td><td>${v.opp_count}</td><td>${v.fills}</td><td>${fmtCur(v.gross_pnl)}</td><td class="${sClass}">${fmtCur(v.net_pnl)}</td>`;
    symBody.appendChild(tr);
  }
}

// Bind sortable symbol table headers
function bindSymbolSort() {
  const cols = ['', 'opp_count', 'fills', 'gross_pnl', 'net_pnl'];
  const ths = $('symbolTable').querySelectorAll('thead th');
  ths.forEach((th, i) => {
    if (!cols[i]) return;
    th.style.cursor = 'pointer';
    th.title = 'Click to sort';
    th.addEventListener('click', () => {
      if (_sortCol === cols[i]) _sortAsc = !_sortAsc;
      else { _sortCol = cols[i]; _sortAsc = false; }
      ths.forEach(t => t.style.color = '');
      th.style.color = '#58a6ff';
      renderSymbolTable(_symbolData, _sortCol, _sortAsc);
    });
  });
}

function loadLedger(text) {
  const lines = text.trim().split(/\r?\n/).filter(Boolean);
  const rows = lines.map(l => { try { return JSON.parse(l); } catch(_) { return null; } }).filter(Boolean);
  const tbody = $('tradesTable').querySelector('tbody');
  tbody.innerHTML = '';
  let cum = 0, labels = [], data = [], minPnL = Infinity, maxPnL = -Infinity;
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
    // Color-coded row background for positive/negative trades
    tr.style.background = pnl >= 0 ? 'rgba(63,185,80,0.04)' : 'rgba(248,81,73,0.05)';
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
  updateDropzoneTimestamp();
}

/* ---------- Dropzone timestamp ---------- */
function updateDropzoneTimestamp() {
  const dz = $('dropzone');
  const ts = new Date().toLocaleTimeString();
  const existing = dz.querySelector('.dz-timestamp');
  if (existing) existing.remove();
  const el = document.createElement('div');
  el.className = 'dz-timestamp';
  el.style.cssText = 'font-size:11px;margin-top:8px;color:#8b949e;';
  el.textContent = 'Last loaded: ' + ts;
  dz.appendChild(el);
}

/* ---------- Charts ---------- */
function renderPnlChart(labels, data) {
  const ctx = $('pnlChart').getContext('2d');
  if (pnlChart) pnlChart.destroy();
  // Compute min for drawdown visibility
  const minVal = Math.min(0, ...data);
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
        y: {
          ticks: { color: '#8b949e', callback: v => '$' + v.toFixed(2) },
          grid: { color: '#21262d' },
          // FIX: suggestedMin: 0 forces baseline at 0 so drawdown is visible below
          suggestedMin: minVal < 0 ? minVal * 1.1 : 0
        }
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

  // FIX: fallback placeholder when no data
  if (rows.length === 0 || (net === 0 && totalFees === 0)) {
    feeChart = new Chart(ctx, {
      type: 'doughnut',
      data: {
        labels: ['Load ledger for breakdown'],
        datasets: [{ data: [1], backgroundColor: ['#30363d'], borderColor: '#0b0e14', borderWidth: 3 }]
      },
      options: {
        responsive: true, maintainAspectRatio: false,
        plugins: {
          legend: { position: 'bottom', labels: { color: '#8b949e', padding: 20 } },
          tooltip: { enabled: false }
        }
      }
    });
    return;
  }

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
  // FIX: count moved to tooltip only — axis label shows range only (no clutter)
  const labels = bins.slice(0, -1).map((b, i) => `$${b}–${bins[i+1]}`);
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
        tooltip: {
          backgroundColor: '#161b22', titleColor: '#c9d1d9', bodyColor: '#c9d1d9',
          borderColor: '#30363d', borderWidth: 1,
          callbacks: { label: c => ` ${c.raw} trades in this range` }
        }
      },
      scales: {
        x: { ticks: { color: '#8b949e', maxRotation: 30 }, grid: { display: false } },
        y: { ticks: { color: '#8b949e', stepSize: 1 }, grid: { color: '#21262d' } }
      }
    }
  });
}

/* ---------- Capital Simulator (FIXED math) ---------- */
function updateCapital() {
  const cap     = parseFloat($('capInput').value)  || 1000;
  const feeBps  = parseFloat($('feeInput').value)  || 26;
  const trades  = parseInt($('tradeInput').value)  || 48;

  const reportText = currentFiles['run_report.json'];
  if (reportText) {
    try {
      const r = JSON.parse(reportText);
      if (r.n_fills && r.n_fills > 0) {
        const avgGross = r.gross_pnl / r.n_fills;
        const avgNet   = r.net_pnl   / r.n_fills;
        const avgFee   = avgGross - avgNet;
        // FIX: scale only by capital ratio — do NOT multiply by (trades/n_fills)
        // which produces nonsense when trades < n_fills.
        // Instead: projected = avgPerTrade * requestedTrades * (cap / baseCap)
        const baseCap = 1000;
        const capScale = cap / baseCap;
        const projectedGross = avgGross * trades * capScale;
        const projectedFees  = avgFee   * trades * capScale;
        const projectedNet   = avgNet   * trades * capScale;
        $('projGross').textContent = fmtCur(projectedGross);
        $('projFees').textContent  = fmtCur(projectedFees);
        $('projNet').textContent   = fmtCur(projectedNet);
        $('projNet').className     = 'value ' + sign(projectedNet);
        $('projPerTrade').textContent = fmtCur(trades > 0 ? projectedNet / trades : 0);
        return;
      }
    } catch(_) {}
  }

  // Fallback simple model
  const projGross = cap * (15 / 10000) * trades;
  const projFees  = cap * (feeBps / 10000) * trades;
  const projNet   = projGross - projFees;
  $('projGross').textContent    = fmtCur(projGross);
  $('projFees').textContent     = fmtCur(projFees);
  $('projNet').textContent      = fmtCur(projNet);
  $('projNet').className        = 'value ' + sign(projNet);
  $('projPerTrade').textContent = fmtCur(trades > 0 ? projNet / trades : 0);
}

['capInput', 'feeInput', 'tradeInput'].forEach(id => {
  $(id).addEventListener('input', updateCapital);
});

/* ---------- File Handling ---------- */
async function readFile(f) {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = e => res(e.target.result);
    r.onerror = rej;
    r.readAsText(f);
  });
}

async function handleFiles(files) {
  let reportText, ledgerText;
  for (const f of Array.from(files)) {
    const text = await readFile(f);
    if (f.name.endsWith('run_report.json'))    { reportText = text; currentFiles['run_report.json'] = text; }
    if (f.name.endsWith('paper_ledger.jsonl')) { ledgerText = text; currentFiles['paper_ledger.jsonl'] = text; }
  }
  if (reportText) { try { loadReport(reportText); } catch(e) { alert('Failed to parse run_report.json: ' + e); } }
  if (ledgerText) { try { loadLedger(ledgerText); } catch(e) { alert('Failed to parse paper_ledger.jsonl: ' + e); } }
}

const dz = $('dropzone');
const fi = $('fileInput');
dz.addEventListener('click', () => fi.click());
dz.addEventListener('dragover', e => { e.preventDefault(); dz.classList.add('dragover'); });
dz.addEventListener('dragleave', () => dz.classList.remove('dragover'));
dz.addEventListener('drop', async e => {
  e.preventDefault(); dz.classList.remove('dragover');
  await handleFiles(Array.from(e.dataTransfer.files));
});
fi.addEventListener('change', async e => { await handleFiles(Array.from(e.target.files)); });

/* ---------- Auto-load sample data ---------- */
(async () => {
  const reportCandidates = [
    '../run_artifacts/latest_backtest/run_report.json',
    '../run_artifacts/latest_backtest_1bps/run_report.json',
    '../run_artifacts/rd_1000_0.5/run_report.json',
    './sample_data/run_report.json',
  ];
  const ledgerCandidates = [
    '../run_artifacts/latest_backtest/paper_ledger.jsonl',
    '../run_artifacts/latest_backtest_1bps/paper_ledger.jsonl',
    '../run_artifacts/rd_1000_0.5/paper_ledger.jsonl',
    './sample_data/paper_ledger.jsonl',
  ];

  for (const path of reportCandidates) {
    try {
      const r = await fetch(path).then(r => r.ok ? r.text() : null).catch(() => null);
      if (r) { loadReport(r); break; }
    } catch(_) {}
  }
  for (const path of ledgerCandidates) {
    try {
      const l = await fetch(path).then(r => r.ok ? r.text() : null).catch(() => null);
      if (l) { loadLedger(l); break; }
    } catch(_) {}
  }
})();

/* ---------- Init ---------- */
bindSymbolSort();
