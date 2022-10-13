use crate::CACHEDIR;
use anyhow::{Context, Result};
use ijson::IString;
use log::info;
use serde::{Deserialize, Serialize};
use sqlx::{migrate::MigrateDatabase, Sqlite, SqlitePool, QueryBuilder};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::Write,
    path::Path,
    process::Command,
};

use super::{nixos::{self, getnixospkgs}, NixPkg, NixPkgList};

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
        if prevver.eq(nixosversion)
            && Path::new(&format!("{}/flakespkgs.db", &*CACHEDIR)).exists()
        {
            info!("No new version of NixOS flakes found");
            return Ok(format!("{}/flakespkgs.db", &*CACHEDIR));
        }
    }

    // Get list of packages from flake
    let pkgsout = if let Some(rev) = version.get("nixpkgsRevision") {
        Command::new("nix")
            .arg("search")
            .arg("--json")
            .arg(&format!("nixpkgs/{}", rev))
            .output()?
    } else {
        Command::new("nix")
            .arg("search")
            .arg("--json")
            // .arg("--inputs-from")
            // .arg(&flakepath)
            .arg("nixpkgs")
            .output()?
    };

    let mut pkgsjson: HashMap<String, NixPkg> =
        serde_json::from_str(&String::from_utf8(pkgsout.stdout)?)?;
    pkgsjson = pkgsjson
        .iter()
        .map(|(k, v)| {
            (
                k.split('.').collect::<Vec<_>>()[2..].join("."),
                v.clone(),
            )
        })
        .collect::<HashMap<_, _>>();

    let dbfile = format!("{}/flakespkgs.db", &*CACHEDIR);
    nixos::createdb(&dbfile, &NixPkgList { packages: pkgsjson }).await?;

    // Write version downloaded to file
    File::create(format!("{}/flakespkgs.ver", &*CACHEDIR))?.write_all(nixosversion.as_bytes())?;

    Ok(format!("{}/flakespkgs.db", &*CACHEDIR))
}

/// Returns a list of all installed system packages with their attribute and version
/// The input `paths` should be the paths to the `configuration.nix` files containing `environment.systemPackages`
pub async fn getflakepkgs(paths: &[&str]) -> Result<HashMap<String, String>> {
    getnixospkgs(paths, nixos::NixosType::Flake).await
}
