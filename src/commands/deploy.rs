use std::collections::HashMap;

use crate::{
    challenge::{Challenge, Expose},
    fly, Commands, Config, DOCKER,
};
use anyhow::{anyhow, Result};
use bollard::{auth::DockerCredentials, image::BuildImageOptions};
use futures::stream::StreamExt;
use serde_json::json;

pub async fn command(config: Config, challs: Option<Vec<String>>) -> Result<()> {
    fly::ensure_app(&config.fly)?;
    let app_name = &config.fly.app_name;
    let challs = if let Some(challs) = challs {
        Challenge::get_some(&config.chall_root, challs)?
    } else {
        Challenge::get_all(&config.chall_root)?
    };
    match challs.len() {
        1 => println!("Deploying {}", challs[0].id),
        2 => println!("Deploying {} and {}", challs[0].id, challs[1].id),
        _ => {
            println!(
                "Deploying {}, {} and {}",
                challs[0].id,
                challs[1].id,
                if challs.len() > 3 {
                    "more"
                } else {
                    &challs[2].id
                },
            )
        }
    }
    let repo = &format!("registry.fly.io/{}", app_name);
    let mut machines = fly::list_machines(app_name)?
        .into_iter()
        .map(|machine| (machine.name.clone(), machine))
        .collect::<HashMap<String, fly::MachineInfo>>();
    println!("{:#?}", machines);

    let mut http_expose: HashMap<String, String> = HashMap::new();
    let mut tcp_expose: HashMap<u32, (String, String)> = HashMap::new();
    for chall in challs {
        chall.push(&repo).await?;
        for (name, container) in &chall.containers {
            let id = chall.container_id(&name);
            println!(
                "{:#?}",
                DOCKER.inspect_image(&format!("{repo}:{id}")).await?
            );
            if container.limits.mem.unwrap_or_default() % 256 != 0 {
                Err(anyhow!("Memory must be a multiple of 256."))?;
            }
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

            let machine = if machines.contains_key(&id) {
                fly::update_machine(app_name, &machines.get(&id).unwrap().id, &machine_config)?
            } else {
                fly::create_machine(app_name, &id, &machine_config)?
            };
            let machine_id = machine.id;
            let internal_url = format!("{machine_id}.vm.{}.internal", config.fly.app_name);
            if let Some(expose) = chall.expose.get(name) {
                match expose {
                    Expose::Tcp { target, tcp } => {
                        tcp_expose.insert(*tcp, (id.clone(), format!("{internal_url}:{target}")));
                    }
                    Expose::Http { target, http } => {
                        http_expose.insert(http.clone(), format!("{internal_url}:{target}"));
                    }
                }
            }
        }
    }

    if !machines.contains_key("ingress") {
        println!("Caddy server not found. Building and deploying.");
        let machine = build_ingress(app_name, &repo).await?;
        println!("Waiting on ingress to start");
        println!("{:?}", fly::wait_for_machine(app_name, &machine.id));
        println!("Ingress Created");
        machines.insert("ingress".to_string(), machine);
    }

    update_ingress(
        config,
        &machines.get("ingress").unwrap().id,
        &repo,
        http_expose,
        tcp_expose,
    )
    .await?;

    return Ok(());
}

fn merge(a: &mut serde_json::Value, b: &serde_json::Value) {
    match (a, b) {
        (serde_json::Value::Object(a), serde_json::Value::Object(b)) => {
            for (k, v) in b {
                merge(a.entry(k.clone()).or_insert(serde_json::Value::Null), v);
            }
        }
        (a, b) => *a = b.clone(),
    }
}
// TODO merge these two functions into one
async fn build_ingress(app_name: &str, repo: &str) -> Result<fly::MachineInfo> {
    let tag = format!("{repo}:ingress");
    let ingress_tar = include_bytes!("../../caddy.tar.gz").to_vec();
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

    let machine = fly::create_machine(
        app_name,
        "ingress",
        &fly::MachineConfig {
            image: tag,
            services: None,
            ..Default::default()
        },
    )?;

    Ok(machine)
}

async fn update_ingress(
    config: Config,
    ingress_id: &str,
    repo: &str,
    http_expose: HashMap<String, String>,
    tcp_expose: HashMap<u32, (String, String)>,
) -> Result<()> {
    let tag = format!("{repo}:ingress");
    let mut services = Vec::new();
    for (port, _) in &tcp_expose {
        services.push(fly::MachineService {
            ports: vec![fly::MachinePort {
                port: Some(*port),
                ..Default::default()
            }],
            protocol: "tcp".to_string(),
            internal_port: *port,
        });
    }

    services.push(fly::MachineService {
        ports: vec![fly::MachinePort {
            port: Some(80),
            ..Default::default()
        }],
        protocol: "tcp".to_string(),
        internal_port: 80,
    });

    services.push(fly::MachineService {
        ports: vec![fly::MachinePort {
            port: Some(443),
            ..Default::default()
        }],
        protocol: "tcp".to_string(),
        internal_port: 443,
    });

    println!(
        "{:#?}",
        fly::update_machine(
            &config.fly.app_name,
            ingress_id,
            &fly::MachineConfig {
                image: tag,
                services: Some(services.into()),
                ..Default::default()
            },
        )?
    );

    println!("Waiting on ingress to start");
    fly::wait_for_machine(&config.fly.app_name, &ingress_id)?;
    println!("Ingress Updated");
    let mut http_expose_json = Vec::with_capacity(http_expose.len());
    for (sub, target) in http_expose {
        http_expose_json.push(json!({
            "match": [{
                "host": [format!("{sub}.{}", config.hostname)],
            }],
            "handle": [{
                "handler": "reverse_proxy",
                "upstreams": [{
                    "dial": target,
                }]
            }]
        }));
    }

    http_expose_json.push(json!({
        "handle": [{
            "handler": "static_response",
            "status_code": 404,
            "body": "Not Found",
        }]
    }));

    let mut tcp_expose_json = HashMap::with_capacity(tcp_expose.len());
    for (port, (name, target)) in tcp_expose {
        tcp_expose_json.insert(
            name,
            json!({
                "listen": [format!("0.0.0.0:{port}")],
                "routes": [{
                    "handle": [
                        {
                            "handler": "proxy",
                            "upstreams": [{ "dial": [target] }]
                        }
                    ]
                }]
            }),
        );
    }

    // okay now we do caddy stuffing
    let mut caddy = json!({
        "apps": {
            "layer4": {
                "servers": tcp_expose_json,
            },
            "http":{
                "servers": {
                    "bear-cds-http": {
                        "listen": [":80"],
                        "routes": [{
                            "handle": [{
                                "handler": "subroute",
                                "routes": http_expose_json,
                            }],
                            "match": [{
                                "host": [format!("*.{}", config.hostname)],
                            }]
                        }],
                    }
                }
            }
        }
    });
    merge(&mut caddy, &config.caddy);
    println!("{}", caddy);
    let mut buf = Vec::new();
    fly::execute_command(
        &config.fly.app_name,
        ingress_id,
        vec![
            "curl",
            "localhost:2019/load",
            "-H",
            "Content-Type: application/json",
            "-d",
            &caddy.to_string(),
        ],
    )?
    .read_to_end(&mut buf)?;
    println!("{:?}", String::from_utf8(buf));
    Ok(())
}
