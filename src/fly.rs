use anyhow::{anyhow, Result};
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

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct MachineConfig {
    pub image: String,
    pub auto_destroy: Option<bool>,
    pub env: Option<HashMap<String, String>>,
    pub guest: Option<AllocatedResources>,
    pub services: Option<Vec<MachineService>>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct MachineService {
    // TODO make type with "udp" and "tcp" as arguments
    pub protocol: String,
    pub internal_port: u32,
    pub ports: Vec<MachinePort>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct MachinePort {
    pub port: Option<u32>,
    pub start_port: Option<u32>,
    pub end_port: Option<u32>,
    pub handlers: Option<Vec<String>>,
}

#[derive(Deserialize, Serialize, Debug, Default)]
pub struct AllocatedResources {
    pub cpu_kind: String,
    pub cpus: Option<u32>,
    pub kernel_args: Option<Vec<String>>,
    pub memory_mb: Option<u32>,
}

#[derive(Deserialize, Debug)]
pub struct MachineInfo {
    pub id: String,
    pub name: String,
    pub state: String,
    pub region: String,
    pub image_ref: MachineImageDetails,
    pub config: MachineConfig,
}

#[derive(Deserialize, Debug)]
pub struct MachineImageDetails {
    pub registry: String,
    pub repository: String,
    pub tag: String,
    pub digest: String,
}

#[derive(Deserialize)]
pub struct AppInfo {
    pub id: String,
    pub name: String,
    pub status: String,
}

fn create_app(name: &str, org: &str) -> Result<AppInfo> {
    let app: AppInfo = ureq::post(&format!("{}/v1/apps", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "app_name": name,
            "org_slug": org,
        }))?
        .into_json()?;
    return Ok(app);
}

fn get_app(name: &str) -> Result<AppInfo> {
    let machine = ureq::get(&format!("{}/v1/apps/{name}", *FLY_HOSTNAME))
        .set("Authorization", &AUTH_HEADER)
        .call()?
        .into_json()?;
    return Ok(machine);
}

pub fn ensure_app(config: &Config) -> Result<AppInfo> {
    let app = get_app(&config.app_name);
    if let Err(ref e) = app {
        if let Some(ureq::Error::Status(404, _)) = e.downcast_ref::<ureq::Error>() {
            eprintln!("App {} not found. Creating...", config.app_name);
            return create_app(&config.app_name, &config.org);
        }
    }
    return app;
}

pub fn create_machine(
    app: &str,
    name: &str,
    machine_config: &MachineConfig,
) -> Result<MachineInfo> {
    let url = format!("{}/v1/apps/{}/machines", *FLY_HOSTNAME, app);

    let json = ureq::post(&url)
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "name": name,
            "config": machine_config,
        }))
        .map_err(|err| {
            anyhow!(
                "Create machine failed: {:?}",
                err.into_response().unwrap().into_string()
            )
        })?
        .into_json()?;

    Ok(json)
}

pub fn update_machine(app: &str, id: &str, machine_config: &MachineConfig) -> Result<MachineInfo> {
    let url = format!("{}/v1/apps/{app}/machines/{id}", *FLY_HOSTNAME);

    let json = ureq::post(&url)
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "config": machine_config,
        }))
        .map_err(|err| {
            anyhow!(
                "{:?}",
                match err.into_response() {
                    Some(resp) => resp.into_string(),
                    None => Ok(String::from("None???")),
                }
            )
        })?
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
    .into_string()?;
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

pub fn list_machines(app: &str) -> Result<Vec<MachineInfo>> {
    let machines = ureq::get(&format!("{}/v1/apps/{}/machines", *FLY_HOSTNAME, app))
        .set("Authorization", &AUTH_HEADER)
        .call()?
        .into_json()?;
    return Ok(machines);
}
