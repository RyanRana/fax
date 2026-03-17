use clap::{Parser, Subcommand};
use tracing_subscriber::EnvFilter;

mod commands;

#[derive(Parser)]
#[command(name = "fax", about = "FAX — Fast Agent Exchange Network", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new agent identity (DID + Ed25519 keypair + EVM address).
    Identity {
        /// Domain for the DID (e.g., "example.com").
        #[arg(short, long)]
        domain: String,
        /// Agent name.
        #[arg(short, long)]
        name: String,
    },

    /// Run a full simulated trade between two agents.
    Demo,

    /// Show RCU conversion rates for all resource types.
    Rates,

    /// Simulate a reputation check for an agent.
    Reputation {
        /// Number of successful trades to simulate.
        #[arg(long, default_value = "10")]
        trades: u64,
        /// Number of disputes to simulate.
        #[arg(long, default_value = "0")]
        disputes: u64,
    },
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("fax=info".parse().unwrap()))
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Identity { domain, name } => commands::identity(&domain, &name),
        Commands::Demo => commands::demo().await,
        Commands::Rates => commands::rates(),
        Commands::Reputation { trades, disputes } => commands::reputation(trades, disputes),
    }
}
