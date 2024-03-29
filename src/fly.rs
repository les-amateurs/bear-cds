use anyhow::Result;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::HashMap, env, io::Read, *};
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
    let json: Value = ureq::post(&format!("{}/v1/apps", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "app_name": name,
            "org_slug": org,
        }))?
        .into_json::<serde_json::Value>()?;
    return Ok(json["id"].as_str().unwrap().to_string());
}

fn get_app(name: &str) -> Result<String> {
    let json: Value = ureq::get(&format!("{}/v1/apps/{name}", *FLY_HOSTNAME))
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

#[derive(Serialize, Debug, Default)]
pub struct MachineConfig {
    pub image: String,
    pub auto_destroy: Option<bool>,
    pub env: Option<HashMap<String, String>>,
    pub guest: Option<AllocatedResources>,
    // TODO this is called, lazyness, and needs to be resolved asap!
    pub services: Option<Value>,
}

#[derive(Serialize, Debug, Default)]
pub struct AllocatedResources {
    pub cpu_kind: String,
    pub cpus: Option<u32>,
    pub kernel_args: Option<Vec<String>>,
    pub memory_mb: Option<u32>,
}

pub fn create_machine(app: &str, name: &str, machine_config: &MachineConfig) -> Result<Value> {
    let url = format!("{}/v1/apps/{}/machines", *FLY_HOSTNAME, app);

    let json = ureq::post(&url)
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "name": name,
            "config": machine_config,
        }))?
        .into_json()?;

    Ok(json)
}

pub fn update_machine(app: &str, id: &str, machine_config: &MachineConfig) -> Result<Value> {
    let url = format!("{}/v1/apps/{app}/machines/{id}", *FLY_HOSTNAME);

    let json = ureq::post(&url)
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "config": machine_config,
        }))?
        .into_json()?;

    Ok(json)
}

pub fn wait_for_machine(app: &str, id: &str) -> Result<()> {
    ureq::get(&format!(
        "{}/v1/apps/{app}/machines/{id}/wait",
        *FLY_HOSTNAME
    ))
    .set("Authorization", &AUTH_HEADER)
    .call()?
    .into_json()?;
    Ok(())
}

pub fn execute_command(
    app: &str,
    id: &str,
    command: Vec<&str>,
) -> Result<Box<dyn Read + Send + Sync + 'static>> {
    Ok(ureq::post(&format!(
        "{}/v1/apps/{app}/machines/{id}/exec",
        *FLY_HOSTNAME
    ))
    .set("Authorization", &AUTH_HEADER)
    .send_json(ureq::json!({
        "command": command
    }))?
    .into_reader())
}

pub fn machines_name_to_id(app: &str) -> Result<HashMap<String, String>> {
    let json = ureq::get(&format!("{}/v1/apps/{}/machines", *FLY_HOSTNAME, app))
        .set("Authorization", &AUTH_HEADER)
        .call()?
        .into_json()?;
    if let Value::Array(arr) = json {
        let mut map = HashMap::with_capacity(arr.len());
        for machine in arr {
            map.insert(
                machine["name"].as_str().unwrap().to_string(),
                machine["id"].as_str().unwrap().to_string(),
            );
        }
        return Ok(map);
    }
    unreachable!()
}
