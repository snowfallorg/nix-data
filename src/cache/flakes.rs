use crate::CACHEDIR;
use anyhow::{Context, Result};
use log::info;
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::Command,
};

use super::{
    nixos::{self, getnixospkgs, nixospkgs},
    NixPkg,
};

/// Gets a list of all packages in the NixOS system with their name and version.
/// Can be used to find what versions of system packages are currently installed.
/// Will only work on NixOS systems.
pub async fn flakespkgs() -> Result<String> {
    let versionout = Command::new("nixos-version").arg("--json").output()?;
    let version: HashMap<String, String> = serde_json::from_slice(&versionout.stdout)?;

    let nixosversion = version
        .get("nixosVersion")
        .context("No NixOS version found")?;

    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/flakespkgs.ver", &*CACHEDIR)) {
        if prevver.eq(nixosversion) && Path::new(&format!("{}/flakespkgs.db", &*CACHEDIR)).exists()
        {
            info!("No new version of NixOS flakes found");
            return Ok(format!("{}/flakespkgs.db", &*CACHEDIR));
        }
    }

    // Get list of packages from flake
    let pkgsout = if let Some(rev) = version.get("nixpkgsRevision") {
        let url = format!("https://raw.githubusercontent.com/snowflakelinux/nixpkgs-version-data/main/nixos-{}/{}.json.br", nixosversion.get(0..5).context("Invalid NixOS version")?, rev);
        let resp = reqwest::get(&url).await?;
        if resp.status().is_success() {
            let r = resp.bytes().await?;
            let mut br = brotli::Decompressor::new(r.as_ref(), 4096);
            let mut pkgsout = Vec::new();
            br.read_to_end(&mut pkgsout)?;
            let pkgsjson: HashMap<String, String> = serde_json::from_slice(&pkgsout)?;
            pkgsjson
        } else {
            let url = format!("https://raw.githubusercontent.com/snowflakelinux/nixpkgs-version-data/main/nixos-unstable/{}.json.br", rev);
            let resp = reqwest::get(&url).await?;
            if resp.status().is_success() {
                let r = resp.bytes().await?;
                let mut br = brotli::Decompressor::new(r.as_ref(), 4096);
                let mut pkgsout = Vec::new();
                br.read_to_end(&mut pkgsout)?;
                let pkgsjson: HashMap<String, String> = serde_json::from_slice(&pkgsout)?;
                pkgsjson
            } else {
                let pkgsout = Command::new("nix")
                    .arg("search")
                    .arg("--json")
                    .arg(&format!("nixpkgs/{}", rev))
                    .output()?;
                let pkgsjson: HashMap<String, NixPkg> =
                    serde_json::from_str(&String::from_utf8(pkgsout.stdout)?)?;
                let pkgsjson = pkgsjson
                    .iter()
                    .map(|(k, v)| {
                        (
                            k.split('.').collect::<Vec<_>>()[2..].join("."),
                            v.version.to_string(),
                        )
                    })
                    .collect::<HashMap<String, String>>();
                pkgsjson
            }
        }
    } else {
        let pkgsout = Command::new("nix")
            .arg("search")
            .arg("--json")
            // .arg("--inputs-from")
            // .arg(&flakepath)
            .arg("nixpkgs")
            .output()?;
        let pkgsjson: HashMap<String, NixPkg> =
            serde_json::from_str(&String::from_utf8(pkgsout.stdout)?)?;
        let pkgsjson = pkgsjson
            .iter()
            .map(|(k, v)| {
                (
                    k.split('.').collect::<Vec<_>>()[2..].join("."),
                    v.version.to_string(),
                )
            })
            .collect::<HashMap<String, String>>();
        pkgsjson
    };

    let dbfile = format!("{}/flakespkgs.db", &*CACHEDIR);
    nixos::createdb(&dbfile, &pkgsout).await?;

    // Write version downloaded to file
    File::create(format!("{}/flakespkgs.ver", &*CACHEDIR))?.write_all(nixosversion.as_bytes())?;

    Ok(format!("{}/flakespkgs.db", &*CACHEDIR))
}

/// Returns a list of all installed system packages with their attribute and version
/// The input `paths` should be the paths to the `configuration.nix` files containing `environment.systemPackages`
pub async fn getflakepkgs(paths: &[&str]) -> Result<HashMap<String, String>> {
    getnixospkgs(paths, nixos::NixosType::Flake).await
}

pub fn uptodate() -> Result<Option<(String, String)>> {
    let flakesver = fs::read_to_string(&format!("{}/flakespkgs.ver", &*CACHEDIR))?;
    let nixosver = fs::read_to_string(&format!("{}/nixospkgs.ver", &*CACHEDIR))?;
    let flakeslast = flakesver
        .split('.')
        .collect::<Vec<_>>()
        .last()
        .context("Invalid version")?
        .to_string();
    let nixoslast = nixosver
        .split('.')
        .collect::<Vec<_>>()
        .last()
        .context("Invalid version")?
        .to_string();
    if !nixoslast.starts_with(&flakeslast) {
        Ok(Some((flakesver, nixosver)))
    } else {
        Ok(None)
    }
}

pub async fn unavailablepkgs(paths: &[&str]) -> Result<HashMap<String, String>> {
    let versionout = Command::new("nixos-version").arg("--json").output()?;
    let version: HashMap<String, String> = serde_json::from_slice(&versionout.stdout)?;
    let nixpath = if let Some(rev) = version.get("nixpkgsRevision") {
        Command::new("nix")
            .arg("eval")
            .arg(&format!("nixpkgs/{}#path", rev))
            .output()?
            .stdout
    } else {
        Command::new("nix")
            .arg("eval")
            .arg("nixpkgs#path")
            .output()?
            .stdout
    };
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

    let pkgs = {
        let mut allpkgs: HashSet<String> = HashSet::new();
        for path in paths {
            if let Ok(filepkgs) = nix_editor::read::getarrvals(
                &fs::read_to_string(path)?,
                "environment.systemPackages",
            ) {
                let filepkgset = filepkgs
                    .into_iter()
                    .map(|x| x.strip_prefix("pkgs.").unwrap_or(&x).to_string())
                    .collect::<HashSet<_>>();
                allpkgs = allpkgs.union(&filepkgset).map(|x| x.to_string()).collect();
            }
        }
        allpkgs
    };

    let mut unavailable = HashMap::new();
    for pkg in pkgs {
        if aliasesout.contains(&pkg) && Command::new("nix-instantiate")
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
            unavailable.insert(pkg, err.to_string());
        }
    }

    let profilepkgs = getflakepkgs(paths).await?;
    let nixospkgs = nixospkgs().await?;
    let pool = SqlitePool::connect(&format!("sqlite://{}", nixospkgs)).await?;

    for (pkg, _) in profilepkgs {
        let (x, broken, insecure): (String, u8, u8) =
            sqlx::query_as("SELECT attribute,broken,insecure FROM meta WHERE attribute = $1")
                .bind(&pkg)
                .fetch_one(&pool)
                .await?;
        if x != pkg {
            unavailable.insert(
                pkg,
                String::from("Package not found in newer version of nixpkgs"),
            );
        } else if broken == 1 {
            unavailable.insert(pkg, String::from("Package is marked as broken"));
        } else if insecure == 1 {
            unavailable.insert(pkg, String::from("Package is marked as insecure"));
        }
    }
    Ok(unavailable)
}
