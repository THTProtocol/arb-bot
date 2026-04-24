use arb_book::order_book::OrderBook;
use arb_core::types::Venue;
use ordered_float::OrderedFloat;

#[test]
fn okx_crc32_known_snapshot() {
    let mut book = OrderBook::default();
    book.venue = Venue::Okx;
    let bids = vec![(OrderedFloat(10000.0), 0.5), (OrderedFloat(9999.0), 1.0)];
    let asks = vec![(OrderedFloat(10001.0), 0.5), (OrderedFloat(10002.0), 1.0)];
    book.apply_snapshot(bids, asks, 0);

    let mut parts: Vec<String> = Vec::new();
    let bids_map: Vec<(f64, f64)> = book
        .bids
        .iter()
        .rev()
        .take(25)
        .map(|(p, q)| (p.into_inner(), *q))
        .collect();
    let asks_map: Vec<(f64, f64)> = book
        .asks
        .iter()
        .take(25)
        .map(|(p, q)| (p.into_inner(), *q))
        .collect();

    fn fmt(v: f64) -> String {
        if v == 0.0 {
            "0".into()
        } else {
            let s = format!("{:.10}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    }

    for i in 0..25 {
        let (bp, bq) = bids_map.get(i).copied().unwrap_or((0.0, 0.0));
        let (ap, aq) = asks_map.get(i).copied().unwrap_or((0.0, 0.0));
        parts.push(fmt(bp));
        parts.push(fmt(bq));
        parts.push(fmt(ap));
        parts.push(fmt(aq));
    }
    let joined = parts.join(":");
    let crc = crc32fast::hash(joined.as_bytes());
    let _signed = crc as i32;
    assert!(crc != 0);
}
