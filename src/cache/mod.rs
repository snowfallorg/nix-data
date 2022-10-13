use std::collections::HashMap;

use ijson::IString;
use serde::{Deserialize, Serialize};

/// Cache and determine packages installed on legacy NixOS and with `nix-env`
pub mod channel;
/// Cache and determine packages installed on flakes enabled NixOS
pub mod flakes;
/// Cache latest NixOS `packages.json` and `options.json`
pub mod nixos;
/// Cache and determine packages installed with `nix profile`
pub mod profile;

#[derive(Debug, Deserialize)]
pub struct NixPkgList {
    packages: HashMap<String, NixPkg>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NixPkg {
    pname: IString,
    version: IString,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct NixosPkg {
    pname: IString,
    version: IString,
}
