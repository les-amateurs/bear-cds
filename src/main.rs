use anyhow::Result;
use bollard::{auth::DockerCredentials, image::BuildImageOptions, Docker};
use challenge::Challenge;
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenvy;
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, fs, path::PathBuf, process::exit};
use toml;

mod challenge;
mod commands;
mod fly;

lazy_static! {
    pub static ref DOCKER: Docker =
        Docker::connect_with_local_defaults().expect("failed to connect to docker");
}

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
    pub hostname: String,
    #[serde(default = "default_caddy")]
    pub caddy: serde_json::Value,
}

fn default_chall_root() -> PathBuf {
    std::env::current_dir()
        .expect("No challenge root set, attempted to read current directory but failed.")
}

fn default_caddy() -> serde_json::Value {
    json!({})
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

    /// Build all challenges
    Build,

    // Deploy all challenges to fly.io
    Deploy,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _ = dotenvy::dotenv();
    let config_file = match fs::read_to_string(args.config) {
        Ok(f) => f,
        Err(_) => {
            print_error!(
                "{}",
                "Bear.toml not found. Please make sure bear.toml exists in the current directory."
            );
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
        Commands::Deploy => debug(commands::deploy::command(config).await)?,
    }

    Ok(())
}

pub fn debug<T>(res: Result<T>) -> Result<()> {
    if let Err(e) = res {
        if let Ok(ureq::Error::Status(_, response)) = e.downcast() {
            println!("{}", response.into_string()?);
        }
    }
    Ok(())
}
