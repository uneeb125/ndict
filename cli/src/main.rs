mod client;

use anyhow::Result;
use clap::{Parser, Subcommand};
use client::DaemonClient;
use shared::ipc::{Command, Response};

#[derive(Parser)]
#[command(name = "ndict")]
#[command(about = "CLI tool for ndict speech-to-text daemon")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Start,
    Stop,
    Pause,
    Resume,
    Status,
    Test,
    Toggle,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = DaemonClient::new();

    let command = match cli.command {
        Commands::Start => Command::Start,
        Commands::Stop => Command::Stop,
        Commands::Pause => Command::Pause,
        Commands::Resume => Command::Resume,
        Commands::Status => Command::Status,
        Commands::Test => Command::SetLanguage("test".to_string()),
        Commands::Toggle => Command::Toggle,
    };

    match client.send_command(command).await {
        Ok(Response::Ok) => {
            println!("Success");
        }
        Ok(Response::Status(info)) => {
            println!("Status:");
            println!("  Running: {}", info.is_running);
            println!("  Active: {}", info.is_active);
            println!("  Language: {}", info.language);
        }
        Ok(Response::Error(msg)) => {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("Failed to connect to ndictd: {}", e);
            std::process::exit(1);
        }
    }

    Ok(())
}
