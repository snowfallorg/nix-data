/// Cache and determine packages installed on legacy NixOS and with `nix-env`
pub mod channel;
/// Cache and determine packages installed on flakes enabled NixOS
pub mod flakes;
/// Cache latest NixOS `packages.json` and `options.json`
pub mod nixos;
/// Cache and determine packages installed with `nix profile`
pub mod profile;
