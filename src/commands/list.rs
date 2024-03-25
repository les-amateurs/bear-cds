use crate::Config;
use anyhow::Result;
use std::{fs, path::Path, *};

pub fn command(config: Config) -> Result<()> {
    if let Some(chall_root) = config.chall_root {
        for category_entry in fs::read_dir(chall_root)? {
            let category = category_entry?.path();

            let category_name = category.file_name().unwrap().to_str().unwrap();
            if category.is_dir() {
                for chall_entry in fs::read_dir(&category)? {
                    let chall = chall_entry?.path();
                    let chall_name = chall.file_name().unwrap().to_str().unwrap();

                    let path = Path::join(&chall, "challenge.toml");
                    if path.exists() {
                        println!("{}/{}", category_name, chall_name)
                    }
                }
            }
        }
        return Ok(());
    }

    eprintln!(
        "Failed to read challenge directory. Make sure it exists and you have correct permissions"
    );
    std::process::exit(1);
}
