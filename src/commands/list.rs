use crate::{challenge::get_chall_paths, Config};
use anyhow::{anyhow, Result};
use std::{fs, path::PathBuf, *};

pub fn command(config: Config) -> Result<()> {
    if let Some(chall_root) = config.chall_root {
        let challs = get_chall_paths(chall_root)?;
        for chall in challs {
            if let [challenge, category] = &chall
                .iter()
                .rev()
                .take(2)
                .map(|c| c.to_str())
                .collect::<Option<Vec<&str>>>()
                .ok_or(anyhow!("Failed to convert OsStr to Str"))?[..]
            {
                println!("{}/{}", category, challenge);
            }
        }
        return Ok(());
    }

    eprintln!(
        "Failed to read challenge directory. Make sure it exists and you have correct permissions"
    );
    std::process::exit(1);
}
