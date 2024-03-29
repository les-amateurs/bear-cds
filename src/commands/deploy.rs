use std::collections::HashMap;

use crate::{
    challenge::{Challenge, Expose},
    fly, Config, DOCKER,
};
use anyhow::Result;
use bollard::{auth::DockerCredentials, image::BuildImageOptions};
use futures::stream::StreamExt;
use serde_json::json;

pub async fn command(config: Config) -> Result<()> {
    fly::ensure_app(&config.fly)?;
    let app_name = &config.fly.app_name;
    let challs = Challenge::get_all(&config.chall_root)?;
    let repo = &format!("registry.fly.io/{}", app_name);
    let mut machines = fly::machines_name_to_id(app_name)?;

    let mut http_expose: HashMap<String, String> = HashMap::new();
    let mut tcp_expose: HashMap<u32, (String, String)> = HashMap::new();
    for chall in challs {
        chall.push(&repo).await?;
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
                    Expose::Tcp { target, tcp } => {
                        tcp_expose.insert(*tcp, (name.clone(), format!("{internal_url}:{target}")));
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
        let machine_id = build_ingress(app_name, &repo).await?;
        println!("Waiting on ingress to start");
        fly::wait_for_machine(app_name, &machine_id)?;
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
                            "upstreams": [{ "dial": target }]
                        }
                    ]
                }]
            }),
        );
    }

    // okay now we do caddy stuffing
    let mut caddy = json!({
        "apps": {
            "http":{
                "servers": {
                    "bear-cds-http": {
                        "listen": [":80"],
                        "routes": http_expose_json,
                    }
                }
            },
            "layer4": {
                "servers": tcp_expose_json,
            }
        }
    });
    merge(&mut caddy, &config.caddy);
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
            &caddy.to_string(),
        ],
    )?
    .read_to_end(&mut buf)?;
    println!("{:?}", String::from_utf8(buf));

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

async fn build_ingress(app_name: &str, repo: &str) -> Result<String> {
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
                        },
                        {
                            "port": 80,
                        },
                        {
                            "start_port": 10_000,
                            "end_port": 40_000,
                        }
                    ],
                    "protocol": "tcp",
                    "internal_port": 80,
                }
            ])),
            ..Default::default()
        },
    )?;

    Ok(json["id"].as_str().unwrap().to_string())
}
