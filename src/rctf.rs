use anyhow::Result;
use lazy_static::lazy_static;
use serde::Deserialize;
use std::{collections::HashMap, env, path::PathBuf};
use ureq;

use crate::challenge::Challenge;

lazy_static! {
    static ref RCTF_ADMIN_TOKEN: String =
        env::var("RCTF_ADMIN_TOKEN").expect("$RCTF_ADMIN_TOKEN not found");
    static ref AUTH_HEADER: String = format!("Bearer {}", *RCTF_ADMIN_TOKEN);
}

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
}

pub async fn update_chall(url: &str, chall: &Challenge) -> Result<()> {
    let category = chall.id.split("/").nth(0).unwrap();
    let id = format!("bcds-{}", chall.id.replace("/", "-"));
    // TODO handle file uploads here
    ureq::put(&format!("{url}/api/v1/admin/challs/{id}"))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "data": {
                "author": chall.author,
                "category": category,
                "description": chall.description,
                "flag": chall.flag,
                "name": chall.name,
                "points": { "min": 100, "max": 500 },
                "tiebreakEligible": true,
            }
        }))?
        .into_string()?;
    Ok(())
}
