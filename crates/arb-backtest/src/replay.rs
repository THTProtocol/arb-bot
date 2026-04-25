use arb_core::types::{BookUpdate, NormalizedSymbol, Venue};
use ordered_float::OrderedFloat;
use serde_json::Value;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

pub fn replay_from_dir<P: AsRef<Path>>(dir: P) -> anyhow::Result<Vec<BookUpdate>> {
    let mut entries: Vec<std::path::PathBuf> = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl")
            && path.extension().and_then(|e| e.to_str()) != Some("gz")
        {
            continue;
        }
        entries.push(path);
    }
    entries.sort();

    let mut updates: Vec<BookUpdate> = Vec::with_capacity(64_000);
    for path in &entries {
        let mut buf: Vec<u8> = Vec::new();
        if path.extension().and_then(|e| e.to_str()) == Some("gz") {
            let f = File::open(path)?;
            let mut gz = flate2::read::GzDecoder::new(f);
            gz.read_to_end(&mut buf)?;
        } else {
            let mut f = File::open(path)?;
            f.read_to_end(&mut buf)?;
        }
        let reader = BufReader::new(buf.as_slice());
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            if let Some(u) = parse_line(&line) {
                updates.push(u);
            } else {
                eprintln!("WARN: failed to parse line: {}", line.chars().take(120).collect::<String>());
            }
        }
    }
    // Deterministic sort by ts_ns, then seq
    updates.sort_by(|a, b| a.ts_ns.cmp(&b.ts_ns).then(a.seq.cmp(&b.seq)));
    // Re-stamp contiguous timestamps with sub-nanos so we preserve order deterministically
    for (i, u) in updates.iter_mut().enumerate() {
        u.seq = i as u64;
    }
    Ok(updates)
}

fn parse_line(line: &str) -> Option<BookUpdate> {
    let val: Value = serde_json::from_str(line).ok()?;
    let venue_str = val["venue"].as_str()?;
    let venue = match venue_str {
        "binance" | "Binance" => Venue::Binance,
        "kraken" | "Kraken" => Venue::Kraken,
        "okx" | "Okx" | "OKX" => Venue::Okx,
        _ => return None,
    };
    let sym = val["symbol"].as_str().unwrap_or("");
    let parts: Vec<&str> = sym.split('/').collect();
    let symbol = NormalizedSymbol::new(
        parts.get(0)?.to_string(),
        parts.get(1)?.to_string(),
    );
    let bids = val["bids"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| {
            let a = v.as_array()?;
            let price: f64 = a.get(0)?.as_f64().or_else(|| a.get(0)?.as_str()?.parse().ok())?;
            let qty: f64 = a.get(1)?.as_f64().or_else(|| a.get(1)?.as_str()?.parse().ok())?;
            Some((OrderedFloat(price), qty))
        })
        .collect();
    let asks = val["asks"]
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter_map(|v| {
            let a = v.as_array()?;
            let price: f64 = a.get(0)?.as_f64().or_else(|| a.get(0)?.as_str()?.parse().ok())?;
            let qty: f64 = a.get(1)?.as_f64().or_else(|| a.get(1)?.as_str()?.parse().ok())?;
            Some((OrderedFloat(price), qty))
        })
        .collect();
    let ts_ns = val["ts_ns"].as_u64().unwrap_or(0);
    let seq = val["seq"].as_u64().unwrap_or(0);
    let is_snapshot = val["is_snapshot"].as_bool().unwrap_or(false);
    Some(BookUpdate {
        venue,
        symbol,
        ts_ns,
        seq,
        bids,
        asks,
        is_snapshot,
    })
}

/// Replay at reduced or full speed.
pub fn replay_speed_factor(speed: f64) -> f64 {
    speed.clamp(0.0001, 1_000_000.0)
}
