use crate::CACHEDIR;
use anyhow::{anyhow, Context, Result};
use ijson::IString;
use log::{info, debug};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{BufReader, Write},
    path::Path,
    process::Command,
};

use super::{channel, flakes};

/// Downloads the latest `packages.json` for the system from the NixOS cache and returns the path to the file.
/// Will only work on NixOS systems.
pub fn nixospkgs() -> Result<String> {
    let versionout = Command::new("nixos-version").output()?;
    let numver = &String::from_utf8(versionout.stdout)?[0..5];
    let version = if numver == "22.11" {
        "unstable"
    } else {
        numver
    };

    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let verurl = format!("https://channels.nixos.org/nixos-{}", version);
    let resp = reqwest::blocking::get(&verurl)?;
    let latestnixosver = resp
        .url()
        .path_segments()
        .context("No path segments found")?
        .last()
        .context("Last element not found")?
        .to_string();
    info!("latestnixosver: {}", latestnixosver);
    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/nixospkgs.ver", &*CACHEDIR)) {
        if prevver == latestnixosver
            && Path::new(&format!("{}/nixospkgs.json", &*CACHEDIR)).exists()
        {
            debug!("No new version of NixOS found");
            return Ok(format!("{}/nixospkgs.json", &*CACHEDIR));
        }
    }

    let url = format!(
        "https://channels.nixos.org/nixos-{}/packages.json.br",
        version
    );

    // Download file with reqwest blocking
    let client = reqwest::blocking::Client::builder().brotli(true).build()?;
    let mut resp = client.get(url).send()?;
    if resp.status().is_success() {
        let mut out = File::create(&format!("{}/nixospkgs.json", &*CACHEDIR))?;
        resp.copy_to(&mut out)?;
        // Write version downloaded to file
        File::create(format!("{}/nixospkgs.ver", &*CACHEDIR))?
            .write_all(latestnixosver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download latest packages.json"));
    }

    Ok(format!("{}/nixospkgs.json", &*CACHEDIR))
}

/// Downloads the latest 'options.json' for the system from the NixOS cache and returns the path to the file.
/// Will only work on NixOS systems.
pub fn nixosoptions() -> Result<String> {
    let versionout = Command::new("nixos-version").output()?;
    let numver = &String::from_utf8(versionout.stdout)?[0..5];
    let version = if numver == "22.11" {
        "unstable"
    } else {
        numver
    };

    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let verurl = format!("https://channels.nixos.org/nixos-{}", version);
    let resp = reqwest::blocking::get(&verurl)?;
    let latestnixosver = resp
        .url()
        .path_segments()
        .context("No path segments found")?
        .last()
        .context("Last element not found")?
        .to_string();
    info!("latestnixosver: {}", latestnixosver);
    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/nixosoptions.ver", &*CACHEDIR)) {
        if prevver == latestnixosver
            && Path::new(&format!("{}/nixosoptions.json", &*CACHEDIR)).exists()
        {
            debug!("No new version of NixOS found");
            return Ok(format!("{}/nixosoptions.json", &*CACHEDIR));
        }
    }

    let url = format!(
        "https://channels.nixos.org/nixos-{}/options.json.br",
        version
    );

    // Download file with reqwest blocking
    let client = reqwest::blocking::Client::builder().brotli(true).build()?;
    let mut resp = client.get(url).send()?;
    if resp.status().is_success() {
        let mut out = File::create(&format!("{}/nixosoptions.json", &*CACHEDIR))?;
        resp.copy_to(&mut out)?;
        // Write version downloaded to file
        File::create(format!("{}/nixosoptions.ver", &*CACHEDIR))?
            .write_all(latestnixosver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download latest options.json"));
    }

    Ok(format!("{}/nixosoptions.json", &*CACHEDIR))
}

#[derive(Debug, Deserialize)]
struct NixosPkg {
    version: IString,
}

pub ( super ) enum NixosType {
    Flake,
    Legacy,
}

pub ( super ) fn getnixospkgs(paths: &[&str], nixos: NixosType) -> Result<HashMap<String, String>> {
    let pkgs = {
        let mut allpkgs: HashSet<String> = HashSet::new();
        for path in paths {
            if let Ok(filepkgs) = nix_editor::read::getarrvals(
                &fs::read_to_string(path)?,
                "environment.systemPackages",
            ) {
                let filepkgset = filepkgs.into_iter().collect::<HashSet<_>>();
                allpkgs = allpkgs.union(&filepkgset).map(|x| x.to_string()).collect();
            }
        }
        allpkgs
    };
    let pkgsjson: HashMap<String, NixosPkg> =
        serde_json::from_reader(BufReader::new(File::open(match nixos {
            NixosType::Flake => flakes::flakespkgs()?,
            NixosType::Legacy => channel::legacypkgs()?,
        })?))?;
    let mut out = HashMap::new();
    for pkg in pkgs {
        if let Some(p) = pkgsjson.get(&pkg) {
            out.insert(pkg, p.version.as_str().to_string());
        }
    }
    Ok(out)
}
