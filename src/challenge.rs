use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};
use toml;

#[derive(Deserialize, Debug)]
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

#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Attachment {
    File(PathBuf),
    Named { file: PathBuf, r#as: String },
}

#[derive(Deserialize, Debug)]
pub struct Container {
    build: PathBuf,
    limits: Limits,
    ports: Vec<u16>,
}

// im honestly uncertain what types these should be so im using these
#[derive(Deserialize, Debug)]
pub struct Limits {
    cpu: String,
    mem: String,
}

// some fancy stuff will be needed to do this, serde docs
#[derive(Deserialize, Debug)]
#[serde(untagged)]
pub enum Expose {
    Tcp { target: u16, tcp: u16 },
    Http { target: u16, http: String },
}

impl Challenge {
    pub fn parse(chall_dir: PathBuf) -> Result<Challenge> {
        let file_data = fs::read_to_string(Path::join(&chall_dir, "challenge.toml"))?;
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

    pub fn get_all(root: PathBuf) -> Result<Vec<Challenge>> {
        let paths = get_chall_paths(root)?;
        paths
            .into_iter()
            .map(|path| Challenge::parse(path))
            .collect::<Result<Vec<Challenge>, _>>()
    }
}

pub fn get_chall_paths(root: PathBuf) -> io::Result<Vec<PathBuf>> {
    let mut challenges = Vec::new();
    for category_entry in fs::read_dir(root)? {
        let category = category_entry?.path();

        if category.is_dir() {
            for chall_entry in fs::read_dir(&category)? {
                let chall = chall_entry?.path();
                // TODO you want to parse the challenge.toml for this cause reason

                let path = Path::join(&chall, "challenge.toml");
                if path.exists() {
                    challenges.push(chall);
                }
            }
        }
    }
    return Ok(challenges);
}
