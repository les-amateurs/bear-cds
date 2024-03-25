use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy;
use serde::Deserialize;
use std::{fs, path::PathBuf, process::exit};
use toml;

mod challenge;
mod commands;
mod fly;

#[derive(Deserialize)]
pub struct Config {
    pub fly: fly::Config,
    #[serde(default = "default_chall_root")]
    pub chall_root: Option<PathBuf>,
}

fn default_chall_root() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

#[derive(Parser)]
#[command(version, about = "les amateurs challenge deployment system", long_about = None)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, default_value = "bear.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List all challenges
    List,
    Deploy,
}

fn main() -> Result<()> {
    let args = Args::parse();
    dotenvy::dotenv()?;
    let config_file = match fs::read_to_string(args.config) {
        Ok(f) => f,
        Err(_) => {
            eprintln!("bear.toml not found. Make sure bear.toml is created in the current working directory.");
            exit(1);
        }
    };

    let config: Config = match toml::from_str(&config_file) {
        Ok(c) => c,
        Err(_) => {
            eprintln!("Failed to parse bear.toml, make sure bear.toml is valid.");
            exit(1);
        }
    };

    match args.command {
        Commands::List => commands::list::command(config)?,
        Commands::Deploy => {
            fly::ensure_app(config.fly)?;
            ()
        }
    }

    Ok(())
}
