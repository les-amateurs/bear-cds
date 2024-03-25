use anyhow::Result;
use dotenvy;
use serde::Deserialize;
use std::{fs, process::exit};
use toml;

mod fly;

#[derive(Deserialize)]
struct Config {
    fly: fly::Config,
}

fn main() -> Result<()> {
    dotenvy::dotenv()?;
    let config_file = match fs::read_to_string("bear.toml") {
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

    fly::ensure_app(config.fly)?;

    Ok(())
}
