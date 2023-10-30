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
      generations = mkOption {
        type = with types; nullOr int;
        default = null;
        example = literalExpression ''5'';
        description = lib.mdDoc ''The number of generations to keep when rebuilding the system. Leaving as null or setting to 0 will keep all generations.'';
      };
    };
  };

  config = mkIf cfg.enable {
      environment.etc."nix-data/config.json".source = jsonFormat.generate "config.json" { inherit (cfg) systemconfig flake flakearg generations; };
    };
}
