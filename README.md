<div align="center">

Nix Data
===
[![crates.io][crates badge]][crate]
[![Built with Nix][builtwithnix badge]][builtwithnix]
[![License: MIT][MIT badge]][MIT]

</div>

A set of modules for easily managing Nix and NixOS packages and options.

This crate contains a [cache](./src/cache) module for caching Nix/NixOS packages and options,
such as the latest `packages.json` and `options.json` from the NixOS cache.

This crate also contains a [config](./src/config) module for maintaining a set of important Nix/NixOS details,
such as the location of the users `configuration.nix` file, and whether they are using flakes or not.
This can be useful so that not ever application/utility needs to maintain their own config files and preferences.

# Example
```rust
extern crate nix_data;
 
fn main() {
    let userpkgs = nix_data::cache::profile::getprofilepkgs_versioned();
    if let Ok(pkgs) = userpkgs {
        println!("List of installed nix profile packages");
        println!("===");
        for (pkg, version) in pkgs {
            println!("{}: {}", pkg, version);
        }
    }
}
```

[crates badge]: https://img.shields.io/crates/v/nix-data.svg?style=for-the-badge
[crate]: https://crates.io/crates/nix-data
[builtwithnix badge]: https://img.shields.io/badge/Built%20With-Nix-41439A?style=for-the-badge&logo=nixos&logoColor=white
[builtwithnix]: https://builtwithnix.org/
[MIT badge]: https://img.shields.io/badge/License-MIT-blue.svg?style=for-the-badge
[MIT]: https://opensource.org/licenses/MIT