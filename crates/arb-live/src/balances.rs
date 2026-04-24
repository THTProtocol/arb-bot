//! Balance helpers for Binance testnet + OKX demo.
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct BinanceBalanceItem {
    pub asset: String,
    pub free: String,
    pub locked: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct BinanceAccount {
    pub balances: Vec<BinanceBalanceItem>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct OkxBalanceDetail {
    pub ccy: String,
    pub availBal: String,
    pub frozenBal: String,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct OkxBalanceData {
    pub details: Vec<OkxBalanceDetail>,
}

#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
pub struct OkxBalanceResp {
    pub code: String,
    pub msg: String,
    pub data: Vec<OkxBalanceData>,
}
