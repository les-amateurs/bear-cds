use anyhow::Result;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};
use toml;

#[derive(Deserialize)]
pub struct Challenge {
    pub name: String,
    pub author: String,
    pub description: String,
    pub flag: String,
    pub provide: Vec<Attachment>,
    pub containers: HashMap<String, Container>,
    pub expose: HashMap<String, Expose>,
}

#[derive(Deserialize)]
pub enum Attachment {
    File(PathBuf),
    Named { file: PathBuf, r#as: String },
}

#[derive(Deserialize)]
pub struct Container {
    build: PathBuf,
    limits: Limits,
    ports: Vec<u16>,
}

// im honestly uncertain what types these should be so im using these
#[derive(Deserialize)]
pub struct Limits {
    cpu: String,
    ram: String,
}

// some fancy stuff will be needed to do this, serde docs
#[derive(Deserialize)]
pub enum Expose {
    Tcp(u16),
    Http(u16),
}

impl Challenge {
    pub fn parse(file: PathBuf) -> Result<Challenge> {
        let file_data = fs::read_to_string(file)?;
        return Ok(toml::from_str(&file_data)?);
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
