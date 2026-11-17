{
  description = "Kioku: a personal second brain for Claude Code";

  inputs.nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
    forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f nixpkgs.legacyPackages.${system});
  in {
    packages = forAllSystems (pkgs: rec {
      kioku = pkgs.callPackage ./nix/package.nix {};
      default = kioku;
    });

    homeManagerModules = rec {
      kioku = import ./nix/hm-module.nix self;
      default = kioku;
    };

    formatter = forAllSystems (pkgs: pkgs.alejandra);
  };
}
