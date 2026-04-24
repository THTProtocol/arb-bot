use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "arb-bot")]
#[command(about = "Cross-exchange arbitrage opportunity detector")]
pub struct Args {
    #[arg(short, long, default_value = "config.yaml")]
    pub config: String,

    #[arg(short, long, default_value = "opportunities.jsonl")]
    pub output: String,

    #[arg(short, long)]
    pub ledger: Option<String>,

    #[arg(long, default_value = "info")]
    pub log_level: String,

    #[arg(long, value_delimiter = ',')]
    pub venues: Option<Vec<String>>,

    #[arg(long)]
    pub mode: Option<String>,

    #[arg(long, env = "BINANCE_TESTNET_API_KEY")]
    pub binance_api_key: Option<String>,

    #[arg(long, env = "BINANCE_TESTNET_API_SECRET")]
    pub binance_api_secret: Option<String>,

    #[arg(long, env = "OKX_DEMO_API_KEY")]
    pub okx_api_key: Option<String>,

    #[arg(long, env = "OKX_DEMO_API_SECRET")]
    pub okx_api_secret: Option<String>,

    #[arg(long, env = "OKX_DEMO_PASSPHRASE")]
    pub okx_passphrase: Option<String>,

    #[arg(long)]
    pub train_input: Option<String>,

    #[arg(long, default_value = "train_report.csv")]
    pub train_output: Option<String>,
    #[arg(long, env = "TELEGRAM_BOT_TOKEN")]
    pub tg_bot_token: Option<String>,

    #[arg(long, env = "TELEGRAM_CHAT_ID")]
    pub tg_chat_id: Option<String>,
}
