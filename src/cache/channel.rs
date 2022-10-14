use crate::CACHEDIR;
use anyhow::{anyhow, Context, Result};
use log::info;
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
    process::Command,
};

use super::{
    nixos::{self, getnixospkgs},
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
    let relver = if nixosversion[0..5].eq("22.11") {
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

    let url = format!(
        "https://releases.nixos.org/nixos/{}/nixos-{}/packages.json.br",
        relver, nixosversion
    );

    // Download file with reqwest blocking
    let client = reqwest::blocking::Client::builder().brotli(true).build()?;
    let resp = client.get(url).send()?;
    if resp.status().is_success() {
        let dbfile = format!("{}/legacypkgs.db", &*CACHEDIR);
        let pkgjson: NixPkgList = serde_json::from_reader(BufReader::new(resp))?;
        nixos::createdb(&dbfile, &pkgjson).await?;
        // Write version downloaded to file
        File::create(format!("{}/legacypkgs.ver", &*CACHEDIR))?
            .write_all(nixosversion.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download legacy packages.json"));
    }

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
