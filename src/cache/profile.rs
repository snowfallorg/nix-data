use crate::{
    cache::{nixos, NixPkgList},
    CACHEDIR,
};
use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use serde::Deserialize;
use sqlx::{Row, SqlitePool};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
    process::Command,
};

#[derive(Debug, Deserialize)]
struct ProfilePkgsRoot {
    elements: Vec<ProfilePkgOut>,
}

#[derive(Debug, Deserialize)]
struct ProfilePkgOut {
    #[serde(rename = "attrPath")]
    attrpath: Option<String>,
    #[serde(rename = "originalUrl")]
    originalurl: Option<String>,
    #[serde(rename = "storePaths")]
    storepaths: Vec<String>,
}

/// Struct containing information about a package installed with `nix profile`.
#[derive(Debug)]
pub struct ProfilePkg {
    pub name: String,
    pub originalurl: String,
}

/// Returns a list of all packages installed with `nix profile` with their name.
/// Does not include individual version.
pub fn getprofilepkgs() -> Result<HashMap<String, ProfilePkg>> {
    let profileroot: ProfilePkgsRoot = serde_json::from_reader(File::open(&format!(
        "{}/.nix-profile/manifest.json",
        std::env::var("HOME")?
    ))?)?;
    let mut out = HashMap::new();
    for pkg in profileroot.elements {
        if let (Some(attrpath), Some(originalurl)) = (pkg.attrpath, pkg.originalurl) {
            let attr = if attrpath.starts_with("legacyPackages") {
                attrpath
                    .split('.')
                    .collect::<Vec<_>>()
                    .get(2..)
                    .context("Failed to get legacyPackage attribute")?
                    .join(".")
            } else {
                format!("{}#{}", originalurl, attrpath)
            };
            if let Some(first) = pkg.storepaths.get(0) {
                let ver = first
                    .get(44..)
                    .context("Failed to get pkg name from store path")?;
                out.insert(
                    attr,
                    ProfilePkg {
                        name: ver.to_string(),
                        originalurl,
                    },
                );
            }
        }
    }
    Ok(out)
}

/// Returns a list of all packages installed with `nix profile` with their name and version.
/// Takes significantly longer than [getprofilepkgs()].
pub async fn getprofilepkgs_versioned() -> Result<HashMap<String, String>> {
    let profilepkgs = getprofilepkgs()?;
    let latestpkgs = if Path::new(&format!("{}/nixpkgs.db", &*CACHEDIR)).exists() {
        format!("{}/nixpkgs.db", &*CACHEDIR)
    } else {
        // Change to something else if overridden
        nixpkgslatest().await?
    };
    let mut out = HashMap::new();
    let pool = SqlitePool::connect(&format!("sqlite://{}", latestpkgs)).await?;
    for (pkg, v) in profilepkgs {
        let mut sqlout = sqlx::query(
            r#"
            SELECT pname FROM pkgs WHERE attribute = $1
            "#,
        )
        .bind(&pkg)
        .fetch_all(&pool)
        .await?;
        if sqlout.len() == 1 {
            let row = sqlout.pop().unwrap();
            let pname: String = row.get("pname");
            if let Some(version) = v.name.strip_prefix(&format!("{}-", pname.as_str())) {
                out.insert(pkg, version.to_string());
            }
        }
    }
    Ok(out)
}

/// Downloads the latest `packages.json` from nixpkgs-unstable
/// and returns the path to the file.
pub async fn nixpkgslatest() -> Result<String> {
    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let mut nixpkgsver = None;
    let regout = Command::new("nix").arg("registry").arg("list").output()?;
    let reg = String::from_utf8(regout.stdout)?.replace("   ", " ");
    for l in reg.split('\n') {
        let parts = l.split(' ').collect::<Vec<_>>();
        if let Some(x) = parts.get(1) {
            if x == &"flake:nixpkgs" {
                if let Some(x) = parts.get(2) {
                    nixpkgsver = Some(x.to_string().replace("github:NixOS/nixpkgs/", ""));
                    break;
                }
            }
        }
    }

    let verurl = if let Some(v) = &nixpkgsver {
        format!("https://channels.nixos.org/{}", v)
    } else {
        String::from("https://channels.nixos.org/nixpkgs-unstable")
    };

    let resp = reqwest::blocking::get(&verurl)?;
    let latestnixpkgsver = resp
        .url()
        .path_segments()
        .context("No path segments found")?
        .last()
        .context("Last element not found")?
        .to_string();
    info!("latestnixpkgsver: {}", latestnixpkgsver);
    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/nixpkgs.ver", &*CACHEDIR)) {
        if prevver == latestnixpkgsver && Path::new(&format!("{}/nixpkgs.db", &*CACHEDIR)).exists()
        {
            debug!("No new version of nixpkgs found");
            return Ok(format!("{}/nixpkgs.db", &*CACHEDIR));
        }
    }

    let url = if let Some(v) = &nixpkgsver {
        format!("https://channels.nixos.org/{}/packages.json.br", v)
    } else {
        String::from("https://channels.nixos.org/nixpkgs-unstable/packages.json.br")
    };
    info!("Downloading {}", url);

    // Download file with reqwest blocking
    let client = reqwest::blocking::Client::builder().brotli(true).build()?;
    let resp = client.get(url).send()?;
    if resp.status().is_success() {
        let dbfile = format!("{}/nixpkgs.db", &*CACHEDIR);
        let pkgjson: NixPkgList = serde_json::from_reader(BufReader::new(resp))?;
        nixos::createdb(&dbfile, &pkgjson).await?;
        // Write version downloaded to file
        File::create(format!("{}/nixpkgs.ver", &*CACHEDIR))?
            .write_all(latestnixpkgsver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download nix profile packages.json"));
    }

    Ok(format!("{}/nixpkgs.db", &*CACHEDIR))
}
