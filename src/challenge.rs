use anyhow::{anyhow, Result};
use bollard::auth::DockerCredentials;
use bollard::image::{BuildImageOptions, PushImageOptions};
use bollard::Docker;
use futures::stream::{self, StreamExt};
use lazy_static::lazy_static;
use serde::Deserialize;
use std::default::Default;
use std::env;
use std::fs::File;
use std::io::Read;
use std::{collections::HashMap, fs, io, path::PathBuf};
use temp_dir::TempDir;
use toml;

#[derive(Deserialize, Debug, Clone)]
pub struct Challenge {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub flag: String,
    pub provide: Vec<Attachment>,
    pub containers: HashMap<String, Container>,
    pub expose: HashMap<String, Expose>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Attachment {
    File(PathBuf),
    Named { file: PathBuf, r#as: String },
}

#[derive(Deserialize, Debug, Clone)]
pub struct Container {
    build: PathBuf,
    limits: Limits,
    ports: Vec<u16>,
}

// im honestly uncertain what types these should be so im using these
#[derive(Deserialize, Debug, Clone)]
pub struct Limits {
    cpu: String,
    mem: String,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Expose {
    Tcp { target: u16, tcp: u16 },
    Http { target: u16, http: String },
}

lazy_static! {
    static ref DOCKER: Docker =
        Docker::connect_with_local_defaults().expect("failed to connect to docker");
    static ref FLY_DOCKER_AUTH: String =
        env::var("FLY_DOCKER_AUTH").expect("FLY_DOCKER_AUTH env variable not set.");
}

impl Challenge {
    pub fn parse(chall_dir: PathBuf) -> Result<Challenge> {
        let file_data = fs::read_to_string(&chall_dir.join("challenge.toml"))?;
        let mut toml: toml::Table = toml::from_str(&file_data)?;
        let mut id_parts = chall_dir
            .iter()
            .rev()
            .take(2)
            .map(|c| c.to_str())
            .collect::<Option<Vec<&str>>>()
            .ok_or(anyhow!("Failed to convert OsStr to Str"))?;
        id_parts.reverse();
        let id = id_parts.join("/");
        toml.insert(String::from("id"), toml::Value::String(id.clone()));
        return Ok(toml
            .try_into()
            .map_err(|e| anyhow!("failed to parse parsing {}: {}", id, e))?);
    }

    pub fn get_all(root: &PathBuf) -> Result<Vec<Challenge>> {
        let paths = get_chall_paths(root)?;
        paths
            .into_iter()
            .map(|path| Challenge::parse(path))
            .collect::<Result<Vec<Challenge>, _>>()
    }

    // TODO return type bad >:(
    pub async fn build(
        self,
        root: &PathBuf,
        tmp_dir: &TempDir,
    ) -> Result<Vec<bollard::models::BuildInfo>> {
        let mut build_info = vec![];
        for (name, container) in &self.containers {
            let mut build_path = root.clone();
            build_path.push(&self.id);
            build_path.push(&container.build);

            // build a tar
            let tar_path =
                tmp_dir.child(format!("{}-{}.docker.tar", self.id.replace("/", "-"), name));
            let tar_file = File::create(&tar_path)?;
            let mut tar = tar::Builder::new(tar_file);
            tar.append_dir_all(".", &build_path)?;
            tar.finish()?;

            let options = BuildImageOptions {
                dockerfile: "Dockerfile",
                t: &format!("{}-{}", self.id.replace("/", "-"), name),
                rm: true,
                ..Default::default()
            };
            let mut read_tar = File::open(&tar_path)?;
            let mut contents = Vec::new();
            read_tar.read_to_end(&mut contents).unwrap();
            println!("building image: {}", options.t);
            let mut build = DOCKER.build_image(options, None, Some(contents.into()));
            while let Some(build_step) = build.next().await {
                if let Some(stream) = build_step?.stream {
                    println!("{stream}")
                }
            }

            // build_info.push();
        }
        Ok(build_info)
    }

    pub async fn build_all(root: PathBuf) -> Result<Vec<Vec<bollard::models::BuildInfo>>> {
        let tmp_dir = TempDir::new().unwrap();
        let challs = Challenge::get_all(&root)?;
        futures::future::join_all(challs.into_iter().map(|chall| chall.build(&root, &tmp_dir)))
            .await
            .into_iter()
            .collect()
    }

    pub async fn push(self) -> Result<()> {
        for (name, container) in &self.containers {
            let mut push = DOCKER.push_image::<String>(
                &format!("{}-{}", self.id.replace("/", "-"), name),
                None,
                Some(DockerCredentials {
                    auth: Some(FLY_DOCKER_AUTH.clone()),
                    serveraddress: Some(String::from("registry.fly.io")),
                    ..Default::default()
                }),
            );

            while let Some(push_step) = push.next().await {
                println!("{:#?}", push_step);
            }
        }
        Ok(())
    }

    pub async fn push_all(root: PathBuf) -> Result<()> {
        let challs = Challenge::get_all(&root)?;
        for chall in challs {
            chall.push().await?;
        }

        Ok(())
    }
}

pub fn get_chall_paths(root: &PathBuf) -> io::Result<Vec<PathBuf>> {
    let mut challenges = Vec::new();
    for category_entry in fs::read_dir(root)? {
        let category = category_entry?.path();

        if category.is_dir() {
            for chall_entry in fs::read_dir(&category)? {
                let chall = chall_entry?.path();
                let path = &chall.join("challenge.toml");
                if path.exists() {
                    challenges.push(chall);
                }
            }
        }
    }
    return Ok(challenges);
}
