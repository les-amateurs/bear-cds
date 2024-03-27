use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenvy;
use serde::Deserialize;
use std::{fs, path::PathBuf, process::exit};
use toml;

mod challenge;
mod commands;
mod fly;

#[macro_export]
macro_rules! print_error {
    ($($arg:tt)*) => ({
        use colored::*;
        eprintln!("{} {}", "ERROR:".red().bold(), format!($($arg)*).red().bold());
    });
}

#[derive(Deserialize)]
pub struct Config {
    pub fly: fly::Config,
    #[serde(default = "default_chall_root")]
    pub chall_root: PathBuf,
}

fn default_chall_root() -> PathBuf {
    std::env::current_dir()
        .expect("No challenge root set, attempted to read current directory but failed.")
}

#[derive(Parser)]
#[command(version, about = "---les amateurs challenge deployment system---", long_about = None)]
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

    #[clap(about = "Build all challenges")]
    Build,

    #[clap(about = "Deploy all challenges to fly.io")]
    Deploy,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _ = dotenvy::dotenv();
    let config_file = match fs::read_to_string(args.config) {
        Ok(f) => f,
        Err(_) => {
            print_error!("{}", "Bear.toml not found. Please make sure bear.toml exists in the current directory.");
            exit(1);
        }
    };

    let config: Config = match toml::from_str(&config_file) {
        Ok(c) => c,
        Err(_) => {
            print_error!("{}", "Failed to parse bear.toml");
            exit(1);
        }
    };

    match args.command {
        Commands::List => commands::list::command(config)?,
        Commands::Build => {
            let res = challenge::Challenge::build_all(config.chall_root).await?;
            println!("{:#?}", res);
            ()
        }
        Commands::Deploy => {
            fly::ensure_app(config.fly)?;
            challenge::Challenge::push_all(config.chall_root).await?;
            ()
        }
    }

    Ok(())
}
