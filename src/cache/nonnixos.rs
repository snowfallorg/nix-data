use crate::CACHEDIR;
use anyhow::{anyhow, Result};
use log::{debug, info};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
};


/// Downloads the latest `packages.json` for the system from the Nix cache and returns the path to an SQLite database `nonnixospkgs.db` which contains package data.
/// Mean for non-NixOS systems.
pub async fn nixpkgs() -> Result<String> {
    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let verurl = String::from(
        "https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixpkgs-unstable/nixpkgs.ver"
    );
    debug!("Checking nixpkgs version");
    let resp = reqwest::get(&verurl).await;
    let resp = if let Ok(r) = resp {
        r
    } else {
        // Internet connection failed
        // Check if we can use the old database
        let dbpath = format!("{}/nonnixospkgs.db", &*CACHEDIR);
        if Path::new(&dbpath).exists() {
            info!("Using old database");
            return Ok(dbpath);
        } else {
            return Err(anyhow!("Could not find latest nixpkgs version"));
        }
    };
    let latestnixpkgsver = if resp.status().is_success() {
        resp.text().await?
    } else {
        return Err(anyhow!("Could not find latest nixpkgs version"));
    };
    debug!("Latest nixpkgs version: {}", latestnixpkgsver);

    let latestnixpkgsver = latestnixpkgsver
        .strip_prefix("nixos-")
        .unwrap_or(&latestnixpkgsver);
    info!("latestnixosver: {}", latestnixpkgsver);
    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/nonnixospkgs.ver", &*CACHEDIR)) {
        if prevver == latestnixpkgsver
            && Path::new(&format!("{}/nonnixospkgs.db", &*CACHEDIR)).exists()
        {
            debug!("No new version of nixpkgs found");
            return Ok(format!("{}/nonnixospkgs.db", &*CACHEDIR));
        }
    }

    let url = String::from(
        "https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixpkgs-unstable/nixpkgs.db.br"
    );
    debug!("Downloading nix-data database");
    let client = reqwest::Client::builder().brotli(true).build()?;
    let resp = client.get(url).send().await?;
    if resp.status().is_success() {
        debug!("Writing nix-data database");
        let mut out = File::create(&format!("{}/nonnixospkgs.db", &*CACHEDIR))?;
        {
            let bytes = resp.bytes().await?;
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
        File::create(format!("{}/nonnixospkgs.ver", &*CACHEDIR))?
            .write_all(latestnixpkgsver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download latest nonnixospkgs.db.br"));
    }
    Ok(format!("{}/nonnixospkgs.db", &*CACHEDIR))
}
