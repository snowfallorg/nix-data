{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    naersk.url = "github:nix-community/naersk";
    flake-compat = {
      url = "github:edolstra/flake-compat";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, naersk, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        naersk-lib = naersk.lib."${system}";
      in rec
      {
        packages.nixeditor = naersk-lib.buildPackage {
          pname = "nix-data";
          root = ./.;
          buildInputs = with pkgs; [
            openssl
            pkg-config
          ];
        };

        defaultPackage = self.packages.${system}.nixeditor;

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
          ];
        };

        nixosModules.nix-data = ({ config, ... }: import ./modules/default.nix {
          inherit pkgs;
          inherit (pkgs) lib;
          inherit config;
        });
        nixosModules.default = nixosModules.nix-data;
      });
}
