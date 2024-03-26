use crate::{challenge::Challenge, Config};
use anyhow::{anyhow, Result};
use std::{fs, path::PathBuf, *};

pub fn command(config: Config) -> Result<()> {
    if let Some(chall_root) = config.chall_root {
        let challs = Challenge::get_all(chall_root)?;
        for chall in challs {
            println!("{:#?}", chall);
        }
        return Ok(());
    }

    eprintln!(
        "Failed to read challenge directory. Make sure it exists and you have correct permissions"
    );
    std::process::exit(1);
}
