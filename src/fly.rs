use anyhow::Result;
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_json;
use std::{env, *};
use thiserror::Error;
use ureq;

lazy_static! {
    static ref FLY_HOSTNAME: String = env::var("FLY_API_HOSTNAME").unwrap();
    static ref AUTH_HEADER: String = format!("Bearer {}", env::var("FLY_API_TOKEN").unwrap());
}

#[derive(Deserialize)]
pub struct Config {
    org: String,
    app_name: String,
}

#[derive(Error, Debug)]
pub enum FlyError {
    #[error("App not found")]
    AppNotFound,
    #[error("Unknown Error: {0}")]
    Generic(String),
}

fn create_app(name: &str, org: &str) -> Result<String> {
    let json: serde_json::Value = ureq::post(&format!("{}/v1/apps", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "app_name": name,
            "org_slug": org,
        }))?
        .into_json::<serde_json::Value>()?;
    return Ok(handle_fly_err(json)?["id"].as_str().unwrap().to_string());
}

fn get_app(name: &str) -> Result<String> {
    let json: serde_json::Value = ureq::get(&format!("{}/v1/apps/{name}", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .call()?
        .into_json()?;
    return Ok(handle_fly_err(json)?["id"].as_str().unwrap().to_string());
}

pub fn ensure_app(config: Config) -> Result<String> {
    let app = get_app(&config.app_name);
    if let Err(ref e) = app {
        if let Some(ureq::Error::Status(404, _)) = e.downcast_ref::<ureq::Error>() {
            eprintln!("App {} not found. Creating...", config.app_name);
            return create_app(&config.app_name, &config.org);
        }
    }
    return app;
}

fn handle_fly_err(json: serde_json::Value) -> std::result::Result<serde_json::Value, FlyError> {
    if json["error"] != serde_json::Value::Null {
        let msg = json["error"].as_str().unwrap();
        if msg.starts_with("Could not find App") {
            return Err(FlyError::AppNotFound);
        }
        return Err(FlyError::Generic(msg.to_string()));
    }
    return Ok(json);
}
