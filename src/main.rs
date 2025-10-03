use base64::{engine::general_purpose, Engine as _};
use clap::{Parser, Subcommand};

use logline_core::identity::LogLineKeyPair;

#[derive(Parser)]
#[command(name = "logline")]
#[command(about = "LogLine Universe - Distributed logging and identity system", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Generate a new LogLine ID
    GenerateId {
        /// Node name for the identity
        #[arg(short, long)]
        node_name: String,
    },
    /// Show version information
    Version,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::GenerateId { node_name } => {
            let keypair = LogLineKeyPair::generate(&node_name, None, None, false);
            println!(
                "Generated LogLine ID: {}",
                keypair
                    .id
                    .to_json()
                    .unwrap_or_else(|_| "Error serializing ID".to_string())
            );
            println!(
                "Public Key: {}",
                general_purpose::STANDARD.encode(keypair.public_key_bytes())
            );
        }
        Commands::Version => {
            println!("LogLine Universe v{}", env!("CARGO_PKG_VERSION"));
            println!("Microservices architecture with WebSocket mesh");
        }
    }

    Ok(())
}
