use anyhow::Result;
use bollard::{auth::DockerCredentials, image::BuildImageOptions, Docker};
use challenge::Challenge;
use clap::{Parser, Subcommand};
use colored::Colorize;
use dotenvy;
use futures::stream::{self, StreamExt};
use lazy_static::lazy_static;
use serde::Deserialize;
use serde_json::json;
use std::{collections::HashMap, fs, path::PathBuf, process::exit};
use toml;

mod challenge;
mod commands;
mod fly;

lazy_static! {
    pub static ref DOCKER: Docker =
        Docker::connect_with_local_defaults().expect("failed to connect to docker");
}

#[macro_export]
macro_rules! print_error {
    ($($arg:tt)*) => ({
        use colored::*;
        eprintln!("{} {}", "ERROR:".red().bold(), format!($($arg)*).red().bold());
    });
}

#[derive(Deserialize)]
pub struct Config {
    pub fly: fly::Config,
    #[serde(default = "default_chall_root")]
    pub chall_root: PathBuf,
    pub hostname: String,
}

fn default_chall_root() -> PathBuf {
    std::env::current_dir()
        .expect("No challenge root set, attempted to read current directory but failed.")
}

#[derive(Parser)]
#[command(version, about = "---les amateurs challenge deployment system---", long_about = None)]
struct Args {
    /// Sets a custom config file
    #[arg(short, long, default_value = "bear.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// List all challenges
    List,

    /// Build all challenges
    Build,

    // Deploy all challenges to fly.io
    Deploy,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let _ = dotenvy::dotenv();
    let config_file = match fs::read_to_string(args.config) {
        Ok(f) => f,
        Err(_) => {
            print_error!(
                "{}",
                "Bear.toml not found. Please make sure bear.toml exists in the current directory."
            );
            exit(1);
        }
    };

    let config: Config = match toml::from_str(&config_file) {
        Ok(c) => c,
        Err(_) => {
            print_error!("{}", "Failed to parse bear.toml");
            exit(1);
        }
    };

    match args.command {
        Commands::List => commands::list::command(config)?,
        Commands::Build => {
            let res = challenge::Challenge::build_all(config.chall_root).await?;
            println!("{:#?}", res);
            ()
        }
        Commands::Deploy => {
            fly::ensure_app(&config.fly)?;
            let app_name = &config.fly.app_name;
            let challs = Challenge::get_all(&config.chall_root)?;
            let repo = &format!("registry.fly.io/{}", app_name);
            let mut machines = fly::machines_name_to_id(app_name)?;

            let mut http_expose: HashMap<String, String> = HashMap::new();
            let mut tcp_expose: HashMap<u32, String> = HashMap::new();
            for chall in challs {
                //chall.push(&repo).await?;
                for (name, container) in &chall.containers {
                    let id = chall.container_id(&name);
                    let machine_config = fly::MachineConfig {
                        image: format!("{repo}:{id}"),
                        guest: Some(fly::AllocatedResources {
                            cpu_kind: "shared".to_string(),
                            cpus: container.limits.cpu,
                            memory_mb: container.limits.mem,
                            kernel_args: None,
                        }),
                        ..Default::default()
                    };

                    let json = if machines.contains_key(&id) {
                        fly::update_machine(app_name, machines.get(&id).unwrap(), &machine_config)?
                    } else {
                        fly::create_machine(app_name, &name, &machine_config)?
                    };
                    let machine_id = json["id"].as_str().unwrap();
                    let internal_url = format!("{machine_id}.vm.{}.internal", config.fly.app_name);
                    if let Some(expose) = chall.expose.get(name) {
                        match expose {
                            challenge::Expose::Tcp { target, tcp } => {
                                tcp_expose.insert(*tcp, format!("{internal_url}:{target}"));
                            }
                            challenge::Expose::Http { target, http } => {
                                http_expose
                                    .insert(http.clone(), format!("{internal_url}:{target}"));
                            }
                        }
                    }
                }
            }

            if !machines.contains_key("ingress") {
                println!("Caddy server not found. Building and deploying.");
                let tag = format!("{repo}:ingress");
                let ingress_tar = include_bytes!("../caddy.tar.gz").to_vec();
                let mut build = DOCKER.build_image(
                    BuildImageOptions {
                        dockerfile: "Dockerfile",
                        t: &tag,
                        rm: true,
                        ..Default::default()
                    },
                    None,
                    Some(ingress_tar.into()),
                );

                while let Some(build_step) = build.next().await {
                    if let Some(stream) = build_step?.stream {
                        println!("{stream}")
                    }
                }

                let mut push = DOCKER.push_image::<String>(
                    &tag,
                    None,
                    Some(DockerCredentials {
                        username: Some("x".to_string()),
                        password: Some(fly::FLY_API_TOKEN.clone()),
                        ..Default::default()
                    }),
                );

                while let Some(push_step) = push.next().await {
                    println!("{:#?}", push_step);
                }

                let json = fly::create_machine(
                    app_name,
                    "ingress",
                    &fly::MachineConfig {
                        image: tag,
                        services: Some(json!([
                            {
                                "ports": [
                                    {
                                        "port": 443,
                                        "handlers": [ "tls", "http"],
                                    },
                                    {
                                        "port": 80,
                                        "handlers": [ "http" ],
                                    }
                                ]
                            }
                        ])),
                        ..Default::default()
                    },
                )?;
                let machine_id = json["id"].as_str().unwrap();
                println!("Waiting on ingress to start");
                debug(fly::wait_for_machine(
                    app_name,
                    json["id"].as_str().unwrap(),
                ))?;
                println!("Ingress Created");
                machines.insert("ingress".to_string(), machine_id.to_string());
            }

            let mut http_expose_json = Vec::with_capacity(http_expose.len());
            for (sub, target) in http_expose {
                http_expose_json.push(json!({
                    "match": [{
                        "host": [
                            format!("{sub}.{}", config.hostname)
                        ]
                    }],
                    "handle": [{
                        "handler": "reverse_proxy",
                        "upstreams": [{
                            "dial": target,
                        }]
                    }]
                }));
            }

            // okay now we do caddy stuffing
            let caddy = json!({
                "apps": {
                    "http":{
                        "servers": {
                            "bear-cds-http": {
                                "listen": ["80"],
                                "routes": http_expose_json,
                            }
                        }
                    }
                }
            })
            .to_string();
            println!("{}", caddy);
            let mut buf = Vec::new();
            fly::execute_command(
                app_name,
                machines.get("ingress").unwrap(),
                vec![
                    "curl",
                    "localhost:2019/load",
                    "-H",
                    "Content-Type: application/json",
                    "-d",
                    &caddy,
                ],
            )?
            .read_to_end(&mut buf)?;
            println!("{:?}", String::from_utf8(buf));
        }
    }

    Ok(())
}

fn debug<T>(res: Result<T>) -> Result<()> {
    if let Err(e) = res {
        if let Ok(ureq::Error::Status(_, response)) = e.downcast() {
            println!("{}", response.into_string()?);
        }
    }
    Ok(())
}
