use crate::{challenge::Challenge, Config};
use anyhow::Result;

pub fn command(config: Config) -> Result<()> {
    let challs = Challenge::get_all(&config.chall_root)?;
    for chall in challs {
        println!("{:#?}", chall);
    }
    return Ok(());
}
