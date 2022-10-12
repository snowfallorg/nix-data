use crate::CACHEDIR;
use anyhow::{Context, Result, anyhow};
use ijson::IString;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{Write, BufReader},
    path::Path,
};
use log::{info, debug};

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

#[derive(Debug, Deserialize)]
struct NixPkgListOut {
    packages: HashMap<IString, NixPkg>
}

#[derive(Debug, Serialize, Deserialize)]
struct NixPkg {
    name: IString,
    pname: IString,
    version: IString,
}

/// Returns a list of all packages installed with `nix profile` with their name and version.
/// Takes significantly longer than [getprofilepkgs()].
pub fn getprofilepkgs_versioned() -> Result<HashMap<String, String>> {
    let profilepkgs = getprofilepkgs()?;
    let latestpkgs = if Path::new(&format!("{}/nixpkgs.json", &*CACHEDIR)).exists() {
        format!("{}/nixpkgs.json", &*CACHEDIR)
    } else {
        // Change to something else if overridden
        nixpkgslatest()?
    };
    let pkgs: HashMap<IString, NixPkg> = serde_json::from_reader(BufReader::new(File::open(latestpkgs)?))?;
    let mut out = HashMap::new();
    for (pkg, v) in profilepkgs {
        if let Some(nixpkg) = pkgs.get(&IString::from(pkg.to_string())) {
            if let Some(version) = v.name.strip_prefix(&format!("{}-", nixpkg.pname.as_str())) {
                out.insert(pkg, version.to_string());
            }
        }
    }
    Ok(out)
}

/// Downloads the latest `packages.json` from nixpkgs-unstable
/// and returns the path to the file.
pub fn nixpkgslatest() -> Result<String> {
    // If cache directory doesn't exist, create it
    if !std::path::Path::new(&*CACHEDIR).exists() {
        std::fs::create_dir_all(&*CACHEDIR)?;
    }

    let verurl = String::from("https://channels.nixos.org/nixpkgs-unstable");
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
        if prevver == latestnixpkgsver && Path::new(&format!("{}/nixpkgs.json", &*CACHEDIR)).exists()
        {
            debug!("No new version of nixpkgs found");
            return Ok(format!("{}/nixpkgs.json", &*CACHEDIR));
        }
    }

    let url = String::from("https://channels.nixos.org/nixpkgs-unstable/packages.json.br");

    // Download file with reqwest blocking
    let client = reqwest::blocking::Client::builder().brotli(true).build()?;
    let resp = client.get(url).send()?;
    if resp.status().is_success() {
        let mut out = File::create(&format!("{}/nixpkgs.json", &*CACHEDIR))?;
        let output: NixPkgListOut = serde_json::from_slice(&resp.bytes()?)?;
        let outjson = serde_json::to_string(&output.packages)?;
        out.write_all(outjson.as_bytes())?;
        // Write version downloaded to file
        File::create(format!("{}/nixpkgs.ver", &*CACHEDIR))?
            .write_all(latestnixpkgsver.as_bytes())?;
    } else {
        return Err(anyhow!("Failed to download nixpkgs-unstable packages.json"))
    }

    Ok(format!("{}/nixpkgs.json", &*CACHEDIR))
}
