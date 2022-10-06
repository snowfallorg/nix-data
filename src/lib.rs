pub mod cache;
pub mod config;

lazy_static::lazy_static! {
    pub static ref CACHEDIR: String = format!("{}/.cache/nix-data/", std::env::var("HOME").unwrap());
    pub static ref CONFIG: String = format!("{}/.config/nix-data/", std::env::var("HOME").unwrap());
}
