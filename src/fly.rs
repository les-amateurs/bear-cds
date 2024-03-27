use anyhow::Result;
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_json;
use std::{env, *};
use ureq;

lazy_static! {
    static ref FLY_HOSTNAME: String = env::var("FLY_API_HOSTNAME").unwrap();
    pub static ref FLY_API_TOKEN: String = env::var("FLY_API_TOKEN").unwrap();
    static ref AUTH_HEADER: String = format!("Bearer {}", *FLY_API_TOKEN);
}

#[derive(Deserialize)]
pub struct Config {
    pub org: String,
    pub app_name: String,
}

fn create_app(name: &str, org: &str) -> Result<String> {
    let json: serde_json::Value = ureq::post(&format!("{}/v1/apps", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "app_name": name,
            "org_slug": org,
        }))?
        .into_json::<serde_json::Value>()?;
    return Ok(json["id"].as_str().unwrap().to_string());
}

fn get_app(name: &str) -> Result<String> {
    let json: serde_json::Value = ureq::get(&format!("{}/v1/apps/{name}", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .call()?
        .into_json()?;
    return Ok(json["id"].as_str().unwrap().to_string());
}

pub fn ensure_app(config: &Config) -> Result<String> {
    let app = get_app(&config.app_name);
    if let Err(ref e) = app {
        if let Some(ureq::Error::Status(404, _)) = e.downcast_ref::<ureq::Error>() {
            eprintln!("App {} not found. Creating...", config.app_name);
            return create_app(&config.app_name, &config.org);
        }
    }
    return app;
}
