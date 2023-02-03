use crate::CACHEDIR;
use anyhow::{anyhow, Context, Result};
use log::{debug, info};
use sqlx::{migrate::MigrateDatabase, Row, Sqlite, SqlitePool};
use std::{
    collections::{HashMap, HashSet},
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    process::{Command, Stdio},
};

use super::{channel, flakes};

/// Downloads the latest `packages.json` for the system from the NixOS cache and returns the path to an SQLite database `nixospkgs.db` which contains package data.
/// Will only work on NixOS systems.
pub async fn nixospkgs() -> Result<String> {
    let versionout = Command::new("nixos-version").output()?;
    let mut version = &String::from_utf8(versionout.stdout)?[0..5];

    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let verurl = format!(
        "https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixos-{}/nixpkgs.ver",
        version
    );
    debug!("Checking NixOS version");
    let resp = reqwest::get(&verurl);
    let resp = if let Ok(r) = resp.await {
        r
    } else {
        // Internet connection failed
        // Check if we can use the old database
        let dbpath = format!("{}/nixospkgs.db", &*CACHEDIR);
        if Path::new(&dbpath).exists() {
            info!("Using old database");
            return Ok(dbpath);
        } else {
            return Err(anyhow!("Could not find latest NixOS version"));
        }
    };
    let latestnixosver = if resp.status().is_success() {
        resp.text().await?
    } else {
        let resp = reqwest::get("https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixos-unstable/nixpkgs.ver").await?;
        if resp.status().is_success() {
            version = "unstable";
            resp.text().await?
        } else {
            return Err(anyhow!("Could not find latest NixOS version"));
        }
    };
    debug!("Latest NixOS version: {}", latestnixosver);

    let latestnixosver = latestnixosver
        .strip_prefix("nixos-")
        .unwrap_or(&latestnixosver);
    info!("latestnixosver: {}", latestnixosver);
    // Check if latest version is already downloaded
    if let Ok(prevver) = fs::read_to_string(&format!("{}/nixospkgs.ver", &*CACHEDIR)) {
        if prevver == latestnixosver && Path::new(&format!("{}/nixospkgs.db", &*CACHEDIR)).exists()
        {
            debug!("No new version of NixOS found");
            return Ok(format!("{}/nixospkgs.db", &*CACHEDIR));
        }
    }

    let url = format!(
        "https://raw.githubusercontent.com/snowflakelinux/nix-data-db/main/nixos-{}/nixpkgs.db.br",
        version
    );
    debug!("Downloading nix-data database");
    let client = reqwest::Client::builder().brotli(true).build()?;
    let resp = client.get(url).send().await?;
    if resp.status().is_success() {
        debug!("Writing nix-data database");
        let mut out = File::create(&format!("{}/nixospkgs.db", &*CACHEDIR))?;
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
        File::create(format!("{}/nixospkgs.ver", &*CACHEDIR))?
            .write_all(latestnixosver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download latest nixospkgs.db.br"));
    }
    Ok(format!("{}/nixospkgs.db", &*CACHEDIR))
}

/// Downloads the latest 'options.json' for the system from the NixOS cache and returns the path to the file.
/// Will only work on NixOS systems.
pub fn nixosoptions() -> Result<String> {
    let versionout = Command::new("nixos-version").output()?;
    let mut version = &String::from_utf8(versionout.stdout)?[0..5];

    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let verurl = format!("https://channels.nixos.org/nixos-{}", version);
    debug!("Checking NixOS version");
    let resp = reqwest::blocking::get(&verurl)?;
    let latestnixosver = if resp.status().is_success() {
        resp.url()
            .path_segments()
            .context("No path segments found")?
            .last()
            .context("Last element not found")?
            .to_string()
    } else {
        let resp = reqwest::blocking::get("https://channels.nixos.org/nixos-unstable")?;
        if resp.status().is_success() {
            version = "unstable";
            resp.url()
                .path_segments()
                .context("No path segments found")?
                .last()
                .context("Last element not found")?
                .to_string()
        } else {
            return Err(anyhow!("Could not find latest NixOS version"));
        }
    };
    debug!("Latest NixOS version: {}", latestnixosver);

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

pub(super) enum NixosType {
    Flake,
    Legacy,
}

pub(super) async fn getnixospkgs(
    paths: &[&str],
    nixos: NixosType,
) -> Result<HashMap<String, String>> {
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
    debug!("getnixospkgs: {:?}", pkgs);
    let pkgsdb = match nixos {
        NixosType::Flake => flakes::flakespkgs().await?,
        NixosType::Legacy => channel::legacypkgs().await?,
    };
    let mut out = HashMap::new();
    let pool = SqlitePool::connect(&format!("sqlite://{}", pkgsdb)).await?;
    for pkg in pkgs {
        let mut sqlout = sqlx::query(
            r#"
            SELECT version FROM pkgs WHERE attribute = $1
            "#,
        )
        .bind(&pkg)
        .fetch_all(&pool)
        .await?;
        if sqlout.len() == 1 {
            let row = sqlout.pop().unwrap();
            let version: String = row.get("version");
            out.insert(pkg, version);
        }
    }
    Ok(out)
}

pub(super) async fn createdb(dbfile: &str, pkgjson: &HashMap<String, String>) -> Result<()> {
    let db = format!("sqlite://{}", dbfile);
    if Path::new(dbfile).exists() {
        fs::remove_file(dbfile)?;
    }
    Sqlite::create_database(&db).await?;
    let pool = SqlitePool::connect(&db).await?;
    sqlx::query(
        r#"
            CREATE TABLE "pkgs" (
                "attribute"	TEXT NOT NULL UNIQUE,
                "version"	TEXT,
                PRIMARY KEY("attribute")
            )
            "#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"
        CREATE UNIQUE INDEX "attributes" ON "pkgs" ("attribute")
        "#,
    )
    .execute(&pool)
    .await?;

    let mut wtr = csv::Writer::from_writer(vec![]);
    for (pkg, version) in pkgjson {
        wtr.serialize((pkg.to_string(), version.to_string()))?;
    }
    let data = String::from_utf8(wtr.into_inner()?)?;
    let mut cmd = Command::new("sqlite3")
        .arg("-csv")
        .arg(dbfile)
        .arg(".import '|cat -' pkgs")
        .stdin(Stdio::piped())
        .spawn()?;
    let cmd_stdin = cmd.stdin.as_mut().unwrap();
    cmd_stdin.write_all(data.as_bytes())?;
    let _status = cmd.wait()?;
    Ok(())
}
