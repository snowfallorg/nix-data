{ config, lib, pkgs, ... }:
with lib;
let
  cfg = config.programs.nix-data;
  jsonFormat = pkgs.formats.json { };
in
{
  options = {
    programs.nix-data = {
      enable = mkEnableOption "nix-data";
      systemconfig = mkOption {
        type = with types; nullOr str;
        example = literalExpression ''"/etc/nixos/configuration.nix"'';
        description = ''Where programs using nix-data looks for your system configuration.'';
      };
      flake = mkOption {
        type = with types; nullOr str;
        default = null;
        example = literalExpression ''"/etc/nixos/flake.nix"'';
        description = ''Where programs using nix-data looks for your system flake file.'';
      };
      flakearg = mkOption {
        type = with types; nullOr str;
        default = null;
        example = literalExpression ''user'';
        description = lib.mdDoc ''The flake argument to use when rebuilding the system. `nixos-rebuild switch --flake $\{programs.nix-data.flake}#$\{programs.nix-data.flakearg}`'';
      };
    };
  };

  config = mkIf (cfg.enable || cfg.systemconfig != null || cfg.flake != null || cfg.flakearg != null) {
      environment.etc."nix-data/config.json".source = jsonFormat.generate "config.json" { inherit (cfg) systemconfig flake flakearg; };
    };
}
