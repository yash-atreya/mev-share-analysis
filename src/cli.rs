use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

impl Cli {
    pub fn parse_args() -> Self {
        Self::parse()
    }
}

#[derive(Subcommand)]
pub enum Commands {
    /// Retrieve MEV-Share events from API endpoint and save to db.
    Events {
        #[arg(long = "block-start")]
        block_start: Option<u64>,
        #[arg(long = "block-end")]
        block_end: Option<u64>,
    },
    /// Scan all existing events in db for landings and refunds onchain.
    ScanRefunds,
}
