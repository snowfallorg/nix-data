
use crate::{CONFIG, SYSCONFIG, CONFIGDIR};
use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Write, BufReader},
    path::Path,
};

/// Struct containing locations of system configuration files and some user configuration.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug, Default)]
pub struct NixDataConfig {
    /// Path to the NixOS configuration file. Typically `/etc/nixos/configuration.nix`.
    pub systemconfig: Option<String>,
    /// Path to the NixOS flake file. Typically `/etc/nixos/flake.nix`.
    pub flake: Option<String>,
    /// Specifies which configuration should be user from the `nixosConfigurations` attribute set in the flake file.
    /// If not set, NixOS defaults to the hostname of the system.
    pub flakearg: Option<String>,
    /// Specifies how many NixOS generations to keep. If set to 0, all generations will be kept.
    /// If not set, the default is 5.
    pub generations: Option<u32>,
}


/// Type of package management used by the user.
/// - [Profile](UserPkgType::Profile) refers to the `nix profile` command.
/// - [Env](UserPkgType::Env) refers to the `nix-env` command.
#[derive(Serialize, Deserialize, PartialEq, Eq, Clone, Debug)]
pub enum UserPkgType {
    Profile,
    Env,
}

/// Reads the config file and returns the config struct.
/// If the config file doesn't exist in both the user (`~/.config/nix-data`) and system (`/etc/nix-data`) config directories,
/// this function will return an error.
pub fn getconfig() -> Result<NixDataConfig> {
    // Check if user config exists
    if Path::new(&*CONFIG).exists() {
        // Read user config
        let config: NixDataConfig = serde_json::from_reader(BufReader::new(File::open(&*CONFIG)?))?;
        Ok(config)
    } else if Path::new(SYSCONFIG).exists() {
        // Read system config
        let config: NixDataConfig = serde_json::from_reader(BufReader::new(File::open(SYSCONFIG)?))?;
        Ok(config)
    }  else {
        Err(anyhow!("No config file found"))
    }
}

/// Writes the config struct to the config file in the user config directory (`~/.config/nix-data`).
pub fn setuserconfig(config: NixDataConfig) -> Result<()> {
    // Check if config directory exists
    if !Path::new(&*CONFIGDIR).exists() {
        fs::create_dir_all(&*CONFIGDIR)?;
    }

    // Write user config
    let mut file = File::create(&*CONFIG)?;
    file.write_all(serde_json::to_string_pretty(&config)?.as_bytes())?;
    Ok(())
}
