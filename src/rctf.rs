use anyhow::{anyhow, Result};
use base64::Engine;
use flate2::{write::GzEncoder, Compression};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs::File, io::Read, path::PathBuf};
use ureq;

use crate::challenge::{Attachment, Challenge, Expose};

lazy_static! {
    static ref RCTF_ADMIN_TOKEN: String =
        env::var("RCTF_ADMIN_TOKEN").expect("$RCTF_ADMIN_TOKEN not found");
    static ref AUTH_HEADER: String = format!("Bearer {}", *RCTF_ADMIN_TOKEN);
}

#[derive(Deserialize)]
pub struct Config {
    pub url: String,
}

pub async fn update_chall(config: &crate::Config, chall: &Challenge) -> Result<()> {
    let rctf = config.rctf.as_ref().unwrap();
    let category = chall.id.split("/").nth(0).unwrap();
    let id = format!("bcds-{}", chall.id.replace("/", "-"));
    let uploaded_files = if let Some(attachments) = &chall.provide {
        let mut files = vec![];
        for attachment in attachments {
            match attachment {
                Attachment::File(f) => {
                    let mut path: PathBuf = config.chall_root.clone();
                    path.push(&chall.id);
                    path.push(f);
                    if path.is_file() {
                        let mut buf = Vec::new();
                        let mut file = File::open(&path)?;
                        file.read_to_end(&mut buf)?;
                        files.push(RctfFile {
                            name: path.file_name().unwrap().to_str().unwrap().to_string(),
                            data: buf,
                        })
                    } else {
                        Err(anyhow!("Provided file {} is not a file.", f.display()))?
                    }
                }
                Attachment::Named { file, r#as } => {
                    let mut path: PathBuf = config.chall_root.clone();
                    path.push(&chall.id);
                    path.push(file);
                    if path.is_file() {
                        let mut buf = Vec::new();
                        let mut file = File::open(&path)?;
                        file.read_to_end(&mut buf)?;
                        files.push(RctfFile {
                            name: r#as.clone(),
                            data: buf,
                        })
                    } else {
                        Err(anyhow!("Provided file {} is not a file.", file.display()))?
                    }
                }
                Attachment::Folder { dir, r#as, exclude } => {
                    let mut path: PathBuf = config.chall_root.clone();
                    path.push(&chall.id);
                    path.push(dir);
                    if path.is_dir() {
                        let filename = dir.file_name().unwrap().to_str().unwrap().to_string();
                        let name = r#as.clone().unwrap_or(filename);
                        let buf = Vec::new();
                        let enc = GzEncoder::new(buf, Compression::default());
                        let mut tar = tar::Builder::new(enc);
                        tar.append_dir_all(format!("./{name}"), &path)?;
                        files.push(RctfFile {
                            name: format!("{name}.tar.gz"),
                            data: tar.into_inner()?.finish()?,
                        });
                    } else {
                        Err(anyhow!("Provided path {} is not a dir", dir.display()))?;
                    }
                }
            }
        }

        upload_files(&rctf.url, files).await?
    } else {
        vec![]
    };

    let mut description = chall.description.clone();
    for (name, expose) in &chall.expose {
        let url = match expose {
            Expose::Tcp { tcp, .. } => format!("`nc chal.{} {tcp}`", config.hostname),
            Expose::Http { http, .. } => format!(
                "[http://{http}.{}](http://{http}.{})",
                config.hostname, config.hostname
            ),
        };
        description = description.replace(&format!("{{{{{name}.url}}}}",), &url);
    }

    ureq::put(&format!("{}/api/v1/admin/challs/{id}", rctf.url))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "data": {
                "author": chall.author,
                "category": category,
                "description": description,
                "flag": chall.get_flag(&config.chall_root)?,
                "name": chall.name,
                "points": { "min": 100, "max": 500 },
                "files": uploaded_files,
                "tiebreakEligible": true,
            }
        }))
        .map_err(|err| {
            anyhow!(
                "Update challenge failed (rctf): {:?}",
                err.into_response().unwrap().into_string()
            )
        })?
        .into_string()?;
    Ok(())
}

pub struct RctfFile {
    name: String,
    data: Vec<u8>,
}

#[derive(Deserialize, Serialize)]
pub struct RctfUploadedFile {
    name: String,
    url: String,
}

// TODO very unprofessional
#[derive(Deserialize)]
pub struct ScrewRustSometimes {
    data: Vec<RctfUploadedFile>,
}

pub async fn upload_files(url: &str, files: Vec<RctfFile>) -> Result<Vec<RctfUploadedFile>> {
    let payload: Vec<serde_json::Value> = files.into_iter().map(|f| ureq::json!({ "name": f.name, "data": format!("data:image/png;base64,{}", base64::engine::general_purpose::URL_SAFE.encode(f.data)) })).collect();
    Ok(ureq::post(&format!("{url}/api/v1/admin/upload"))
        .set("Authorization", &AUTH_HEADER)
        .send_json(ureq::json!({
            "files": payload,
        }))
        .map_err(|err| {
            anyhow!(
                "Upload files failed (rctf): {:?}",
                err.into_response().unwrap().into_string()
            )
        })?
        .into_json::<ScrewRustSometimes>()?
        .data)
}
