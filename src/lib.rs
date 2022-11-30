//! A set of modules for easily managing Nix and NixOS packages and options.
//! 
//! This crate contains a [cache](crate::cache) module for caching Nix/NixOS packages and options,
//! such as the latest `packages.json` and `options.json` from the NixOS cache.
//! 
//! This crate also contains a [config](crate::config) module for maintaining a set of important Nix/NixOS details,
//! such as the location of the users `configuration.nix` file, and whether they are using flakes or not.
//! This can be useful so that not ever application/utility needs to maintain their own config files and preferences.
//! 
//! # Example
//! ```
//! extern crate nix_data;
//!  
//! fn main() {
//!     let userpkgs = nix_data::cache::profile::getprofilepkgs_versioned();
//!     if let Ok(pkgs) = userpkgs {
//!         println!("List of installed nix profile packages");
//!         println!("===");
//!         for (pkg, version) in pkgs {
//!             println!("{}: {}", pkg, version);
//!         }
//!     }
//! }
//! ```

/// A module for downloading and caching lists of Nix/NixOS packages and options.
pub mod cache;
/// A module for managing the configuration containing user and system options.
pub mod config;

pub mod utils;

lazy_static::lazy_static! {
    static ref CACHEDIR: String = format!("{}/.cache/nix-data", std::env::var("HOME").unwrap());
    static ref CONFIGDIR: String = format!("{}/.config/nix-data", std::env::var("HOME").unwrap());
    static ref CONFIG: String = format!("{}/config.json", &*CONFIGDIR);
    static ref HOME: String = std::env::var("HOME").unwrap();
}
static SYSCONFIG: &str = "/etc/nix-data/config.json";
