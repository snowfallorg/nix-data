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
struct NixPkgList {
    packages: HashMap<String, NixPkg>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NixPkg {
    pname: IString,
    version: IString,
}

#[derive(Debug, Deserialize)]
struct NixosPkgList {
    packages: HashMap<String, NixosPkg>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct NixosPkg {
    pname: IString,
    version: IString,
    system: IString,
    meta: Meta,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Meta {
    pub broken: Option<bool>,
    pub insecure: Option<bool>,
    pub unsupported: Option<bool>,
    pub unfree: Option<bool>,
    pub description: Option<IString>,
    #[serde(rename = "longDescription")]
    pub longdescription: Option<IString>,
    pub homepage: Option<StrOrVec>,
    pub maintainers: Option<ijson::IValue>,
    pub position: Option<IString>,
    pub license: Option<LicenseEnum>,
    pub platforms: Option<Platform>
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum StrOrVec {
    Single(IString),
    List(Vec<IString>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
pub enum Platform {
    Single(IString),
    List(Vec<IString>),
    ListList(Vec<Vec<IString>>),
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(untagged)]
enum LicenseEnum {
    Single(License),
    List(Vec<License>),
    SingleStr(IString),
    VecStr(Vec<IString>),
    Mixed(Vec<LicenseEnum>)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct License {
    pub free: Option<bool>,
    #[serde(rename = "fullName")]
    pub fullname: Option<IString>,
    #[serde(rename = "spdxId")]
    pub spdxid: Option<IString>,
    pub url: Option<IString>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
struct PkgMaintainer {
    pub email: Option<IString>,
    pub github: Option<IString>,
    pub matrix: Option<IString>,
    pub name: Option<IString>
}