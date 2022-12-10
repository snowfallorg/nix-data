use crate::{
    cache::{nixos, NixPkgList},
    CACHEDIR,
};
use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use serde::Deserialize;
use sqlx::{Row, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufReader, Write, Read},
    path::Path,
    process::Command,
};

use super::{flakes::getflakepkgs, nixos::nixospkgs};

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
    if !Path::new(&format!("{}/.nix-profile/manifest.json", std::env::var("HOME")?)).exists() {
        return Ok(HashMap::new());
    }
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
    if !Path::new(&format!("{}/.nix-profile/manifest.json", std::env::var("HOME")?)).exists() {
        return Ok(HashMap::new());
    }
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
        format!(
            "https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/{}/nixpkgs.ver",
            v
        )
    } else {
        String::from("https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixpkgs-unstable/nixpkgs.ver")
    };
    debug!("Checking nixpkgs version");
    let resp = reqwest::blocking::get(&verurl);
    let resp = if let Ok(r) = resp {
        r
    } else {
        // Internet connection failed
        // Check if we can use the old database
        let dbpath = format!("{}/nixpkgs.db", &*CACHEDIR);
        if Path::new(&dbpath).exists() {
            info!("Using old database");
            return Ok(dbpath);
        } else {
            return Err(anyhow!("Could not find latest nixpkgs version"));
        }
    };
    let latestnixpkgsver = if resp.status().is_success() {
        resp.text()?
    } else {
        return Err(anyhow!("Could not find latest nixpkgs version"));
    };
    debug!("Latest nixpkgs version: {}", latestnixpkgsver);

    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/nixpkgs.ver", &*CACHEDIR)) {
        if prevver == latestnixpkgsver && Path::new(&format!("{}/nixpkgs.db", &*CACHEDIR)).exists()
        {
            debug!("No new version of nixpkgs found");
            return Ok(format!("{}/nixpkgs.db", &*CACHEDIR));
        }
    }

    let url = if let Some(v) = &nixpkgsver {
        format!(
            "https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/{}/nixpkgs_versions.db.br",
            v
        )
    } else {
        String::from("https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixpkgs-unstable/nixpkgs_versions.db.br")
    };
    debug!("Downloading nix-data database");
    let client = reqwest::blocking::Client::builder().brotli(true).build()?;
    let resp = client.get(url).send()?;
    if resp.status().is_success() {
        debug!("Writing nix-data database");
        let mut out = File::create(&format!("{}/nixpkgs.db", &*CACHEDIR))?;
        {
            let bytes = resp.bytes()?;
            let mut reader = brotli::Decompressor::new(
                bytes.as_ref(),
                4096, // buffer size
            );
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf[..]) {
                    Err(e) => {
                        if let std::io::ErrorKind::Interrupted = e.kind() {
                            continue;
                        }
                        panic!("{}", e);
                    }
                    Ok(size) => {
                        if size == 0 {
                            break;
                        }
                        if let Err(e) = out.write_all(&buf[..size]) {
                            panic!("{}", e)
                        }
                    }
                }
            }
        }
        debug!("Writing nix-data version");
        // Write version downloaded to file
        File::create(format!("{}/nixpkgs.ver", &*CACHEDIR))?
            .write_all(latestnixpkgsver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download latest nixpkgs.db.br"));
    }
    Ok(format!("{}/nixpkgs.db", &*CACHEDIR))
}

pub async fn unavailablepkgs() -> Result<HashMap<String, String>> {
    let nixpath = Command::new("nix")
        .arg("eval")
        .arg("nixpkgs#path")
        .output()?
        .stdout;
    let nixpath = String::from_utf8(nixpath)?;
    let nixpath = nixpath.trim();

    let aliases = Command::new("nix-instantiate")
        .arg("--eval")
        .arg("-E")
        .arg(&format!("with import {} {{}}; builtins.attrNames ((self: super: lib.optionalAttrs config.allowAliases (import {}/pkgs/top-level/aliases.nix lib self super)) {{}} {{}})", nixpath, nixpath))
        .arg("--json")
        .output()?;
    let aliasstr = String::from_utf8(aliases.stdout)?;
    let aliasesout: HashSet<String> = serde_json::from_str(&aliasstr)?;

    let flakespkgs = getprofilepkgs()?;
    let mut unavailable = HashMap::new();
    for pkg in flakespkgs.keys() {
        if aliasesout.contains(pkg) && Command::new("nix-instantiate")
                .arg("--eval")
                .arg("-E")
                .arg(&format!("with import {} {{}}; builtins.tryEval ((self: super: lib.optionalAttrs config.allowAliases (import {}/pkgs/top-level/aliases.nix lib self super)) {{}} {{}}).{}", nixpath, nixpath, pkg))
                .output()?.status.success() {
            let out = Command::new("nix-instantiate")
                .arg("--eval")
                .arg("-E")
                .arg(&format!("with import {} {{}}; ((self: super: lib.optionalAttrs config.allowAliases (import {}/pkgs/top-level/aliases.nix lib self super)) {{}} {{}}).{}", nixpath, nixpath, pkg))
                .output()?;
            let err = String::from_utf8(out.stderr)?;
            let err = err.strip_prefix("error: ").unwrap_or(&err).trim();
            unavailable.insert(pkg.to_string(), err.to_string());
        }
    }

    let nixospkgs = nixospkgs().await?;
    let pool = SqlitePool::connect(&format!("sqlite://{}", nixospkgs)).await?;

    for pkg in flakespkgs.keys() {
        let (x, broken, insecure): (String, u8, u8) =
            sqlx::query_as("SELECT attribute,broken,insecure FROM meta WHERE attribute = $1")
                .bind(&pkg)
                .fetch_one(&pool)
                .await?;
        if &x != pkg {
            unavailable.insert(
                pkg.to_string(),
                String::from("Package not found in newer version of nixpkgs"),
            );
        } else if broken == 1 {
            unavailable.insert(pkg.to_string(), String::from("Package is marked as broken"));
        } else if insecure == 1 {
            unavailable.insert(
                pkg.to_string(),
                String::from("Package is marked as insecure"),
            );
        }
    }
    Ok(unavailable)
}
