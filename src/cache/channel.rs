use crate::CACHEDIR;
use anyhow::{anyhow, Context, Result};
use log::info;
use serde::Deserialize;
use sqlx::SqlitePool;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::Path,
    process::Command,
};

use super::{
    nixos::{self, getnixospkgs, nixospkgs},
    NixPkgList,
};

/// Gets a list of all packages in legacy NixOS systems with their name and version.
/// Can be used to find what versions of system packages are currently installed.
/// Will only work on legacy NixOS systems.
pub async fn legacypkgs() -> Result<String> {
    let versionout = Command::new("nixos-version").arg("--json").output()?;
    let version: HashMap<String, String> = serde_json::from_slice(&versionout.stdout)?;

    let nixosversion = version
        .get("nixosVersion")
        .context("No NixOS version found")?;
    let relver = if nixosversion[5..8].eq("pre") {
        "unstable"
    } else {
        &nixosversion[0..5]
    };

    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/legacypkgs.ver", &*CACHEDIR)) {
        if prevver.eq(nixosversion) && Path::new(&format!("{}/legacypkgs.db", &*CACHEDIR)).exists()
        {
            info!("No new version of NixOS legacy found");
            return Ok(format!("{}/legacypkgs.db", &*CACHEDIR));
        }
    }

    async fn downloadrelease(relver: &str, nixosversion: &str) -> Result<HashMap<String, String>> {
        let url = format!(
            "https://releases.nixos.org/nixos/{}/nixos-{}/packages.json.br",
            relver, nixosversion
        );
        // Download file with reqwest
        let client = reqwest::Client::builder().brotli(true).build()?;
        let resp = client.get(url).send().await;
        let resp = if let Ok(r) = resp {
            r
        } else {
            return Err(anyhow!("Failed to download legacy packages.json"));
        };
        if resp.status().is_success() {
            let pkgjson: NixPkgList =
                serde_json::from_reader(BufReader::new(resp.text().await?.as_bytes()))?;
            let pkgout = pkgjson
                .packages
                .iter()
                .map(|(k, v)| (k.to_string(), v.version.to_string()))
                .collect::<HashMap<String, String>>();
            Ok(pkgout)
        } else {
            Err(anyhow!("Failed to download legacy packages.json"))
        }
    }

    // Get list of packages
    let pkgout = if let Some(rev) = version.get("nixpkgsRevision") {
        let url = format!("https://raw.githubusercontent.com/snowflakelinux/nixpkgs-version-data/main/nixos-{}/{}.json.br", relver, rev);
        println!("{}", url);
        let resp = reqwest::get(&url).await?;
        if resp.status().is_success() {
            let r = resp.bytes().await?;
            println!("Downloaded");
            let mut br = brotli::Decompressor::new(r.as_ref(), 4096);
            let mut pkgsout = Vec::new();
            br.read_to_end(&mut pkgsout)?;
            let pkgsjson: HashMap<String, String> = serde_json::from_slice(&pkgsout)?;
            println!("Decompressed");
            pkgsjson
        } else {
            let url = format!("https://raw.githubusercontent.com/snowflakelinux/nixpkgs-version-data/main/nixos-unstable/{}.json.br", rev);
            println!("{}", url);
            let resp = reqwest::get(&url).await?;
            if resp.status().is_success() {
                let r = resp.bytes().await?;
                println!("Downloaded");
                let mut br = brotli::Decompressor::new(r.as_ref(), 4096);
                let mut pkgsout = Vec::new();
                br.read_to_end(&mut pkgsout)?;
                let pkgsjson: HashMap<String, String> = serde_json::from_slice(&pkgsout)?;
                println!("Decompressed");
                pkgsjson
            } else {
                downloadrelease(relver, nixosversion).await?
            }
        }
    } else {
        downloadrelease(relver, nixosversion).await?
    };
    let dbfile = format!("{}/legacypkgs.db", &*CACHEDIR);

    nixos::createdb(&dbfile, &pkgout).await?;

    // Write version downloaded to file
    File::create(format!("{}/legacypkgs.ver", &*CACHEDIR))?.write_all(nixosversion.as_bytes())?;

    Ok(format!("{}/legacypkgs.db", &*CACHEDIR))
}

/// Gets a list of all packages in NixOS systems with their attribute and version.
/// The input `paths` should be the paths to the `configuration.nix` files containing `environment.systemPackages`
pub async fn getlegacypkgs(paths: &[&str]) -> Result<HashMap<String, String>> {
    getnixospkgs(paths, nixos::NixosType::Legacy).await
}

#[derive(Debug, Deserialize)]
struct EnvPkgOut {
    pname: String,
    version: String,
}

/// Gets a list of all packages installed with `nix-env` with their name and version.
/// Due to limitations of `nix-env`, the HashMap keys are the packages `pname` rather than `attributePath`.
/// This means that finding more information about the specific derivations is more difficult.
pub fn getenvpkgs() -> Result<HashMap<String, String>> {
    let output = Command::new("nix-env").arg("-q").arg("--json").output()?;
    let pkgs: HashMap<String, EnvPkgOut> = serde_json::from_slice(&output.stdout)?;
    let mut out = HashMap::new();
    for (_, v) in pkgs {
        out.insert(v.pname, v.version);
    }
    Ok(out)
}

pub fn uptodate() -> Result<Option<(String, String)>> {
    let legacyver = fs::read_to_string(&format!("{}/legacypkgs.ver", &*CACHEDIR))?;
    let nixosver = fs::read_to_string(&format!("{}/nixospkgs.ver", &*CACHEDIR))?;
    if !nixosver.eq(&legacyver) {
        Ok(Some((legacyver, nixosver)))
    } else {
        Ok(None)
    }
}

pub async fn unavailablepkgs(paths: &[&str]) -> Result<HashMap<String, String>> {
    let aliases = Command::new("nix-instantiate")
        .arg("--eval")
        .arg("-E")
        .arg("with import <nixpkgs> {}; builtins.attrNames ((self: super: lib.optionalAttrs config.allowAliases (import <nixpkgs/pkgs/top-level/aliases.nix> lib self super)) {} {})")
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
                .arg(&format!("with import <nixpkgs> {{}}; builtins.tryEval ((self: super: lib.optionalAttrs config.allowAliases (import <nixpkgs/pkgs/top-level/aliases.nix> lib self super)) {{}} {{}}).{}", pkg))
                .output()?.status.success() {
            let out = Command::new("nix-instantiate")
                .arg("--eval")
                .arg("-E")
                .arg(&format!("with import <nixpkgs> {{}}; ((self: super: lib.optionalAttrs config.allowAliases (import <nixpkgs/pkgs/top-level/aliases.nix> lib self super)) {{}} {{}}).{}", pkg))
                .output()?;
            let err = String::from_utf8(out.stderr)?;
            let err = err.strip_prefix("error: ").unwrap_or(&err).trim();
            unavailable.insert(pkg, err.to_string());
        }
    }

    let legacypkgs = getlegacypkgs(paths).await?;
    let nixospkgs = nixospkgs().await?;
    let pool = SqlitePool::connect(&format!("sqlite://{}", nixospkgs)).await?;

    for (pkg, _) in legacypkgs {
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
