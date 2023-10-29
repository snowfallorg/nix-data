{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
  };

  outputs = { self, nixpkgs, flake-utils, naersk, ... }:
    rec {
      nixosModules.nix-data = import ./modules/default.nix;
      nixosModules.default = nixosModules.nix-data;
    }
    // flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        naersk-lib = naersk.lib."${system}";
      in
      rec
      {
        devShell = pkgs.mkShell {
          buildInputs = with pkgs; [
            rust-analyzer
            rustc
            rustfmt
            cargo
            cargo-tarpaulin
            clippy
            openssl
            pkg-config
            sqlite
          ];
        };
      });
}
