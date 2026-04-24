use crate::types::Venue;

/// Fee schedule per venue.
#[derive(Debug, Clone, Copy)]
pub struct FeeModel {
    pub binance_taker_bps: f64,
    pub binance_maker_bps: f64,
    pub kraken_taker_bps: f64,
    pub kraken_maker_bps: f64,
    pub okx_taker_bps: f64,
    pub okx_maker_bps: f64,
}

impl FeeModel {
    pub fn taker_bps(&self, venue: Venue) -> f64 {
        match venue {
            Venue::Binance => self.binance_taker_bps,
            Venue::Kraken => self.kraken_taker_bps,
            Venue::Okx => self.okx_taker_bps,
        }
    }

    pub fn maker_bps(&self, venue: Venue) -> f64 {
        match venue {
            Venue::Binance => self.binance_maker_bps,
            Venue::Kraken => self.kraken_maker_bps,
            Venue::Okx => self.okx_maker_bps,
        }
    }

    pub fn fee_fraction(&self, venue: Venue, is_taker: bool) -> f64 {
        let bps = if is_taker {
            self.taker_bps(venue)
        } else {
            self.maker_bps(venue)
        };
        bps / 10_000.0
    }
}
