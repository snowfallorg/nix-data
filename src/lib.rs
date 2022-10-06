/// A module for downloading and caching lists of Nix/NixOS packages and options.
pub mod cache;
/// A module for managing the configuration containing user and system options.
pub mod config;

lazy_static::lazy_static! {
    static ref CACHEDIR: String = format!("{}/.cache/nix-data/", std::env::var("HOME").unwrap());
    static ref CONFIGDIR: String = format!("{}/.config/nix-data/", std::env::var("HOME").unwrap());
    static ref CONFIG: String = format!("{}/config.json", &*CONFIGDIR);
}
static SYSCONFIG: &str = "/etc/nix-data/config.json";
