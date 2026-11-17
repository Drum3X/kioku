_self: {
  config,
  lib,
  pkgs,
  ...
}: let
  cfg = config.programs.kioku;
  inherit (lib) mkEnableOption mkIf mkOption types escapeShellArg;

  # The system files the vault's .kioku link points to: the package, or a checkout.
  src =
    if cfg.source == null
    then "${cfg.package}/share/kioku"
    else cfg.source;
  path = lib.makeBinPath cfg.package.runtimeDeps;

  cli = pkgs.writeShellScriptBin "kioku" ''
    export PATH=${path}:$PATH
    exec ${escapeShellArg cfg.vault}/.kioku/bin/kioku "$@"
  '';
  # install.sh by hand, e.g. kioku-install --uninstall before turning the module off.
  installer = pkgs.writeShellScriptBin "kioku-install" ''
    export PATH=${path}:$PATH
    exec ${escapeShellArg src}/install.sh "$@"
  '';
in {
  options.programs.kioku = {
    enable = mkEnableOption "Kioku, a personal second brain for Claude Code";

    package = mkOption {
      type = types.package;
      # Built from the user's own pkgs, so its tools are the same store paths as theirs
      # (no collisions in home.packages).
      default = pkgs.callPackage ./package.nix {};
      defaultText = lib.literalExpression "pkgs.callPackage ./package.nix {}";
      description = "The Kioku package.";
    };

    vault = mkOption {
      type = types.str;
      default = "${config.home.homeDirectory}/Documents/Kioku";
      defaultText = lib.literalExpression ''"''${config.home.homeDirectory}/Documents/Kioku"'';
      description = "The vault folder. Created on first activation; existing notes are never overwritten.";
    };

    antigravity = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Also set Kioku up in Google Antigravity (rules in ~/.gemini/GEMINI.md, skills in
        ~/.gemini/config/skills) when ~/.gemini exists. Set false to leave Antigravity alone.
      '';
    };

    source = mkOption {
      type = types.nullOr types.str;
      default = null;
      example = "/home/me/Projects/kioku";
      description = ''
        A local checkout of Kioku to use instead of the package, for development. The vault
        links to it, so edits take effect without a rebuild. Leave null to use the package
        (the version pinned in your flake.lock).
      '';
    };
  };

  config = mkIf cfg.enable {
    home.packages = [cli installer] ++ cfg.package.runtimeDeps;

    # The wiring Claude Code needs lives in files it also edits itself (settings.json,
    # CLAUDE.md), so it is merged by install.sh rather than owned by Home Manager.
    home.activation.kioku = lib.hm.dag.entryAfter ["writeBoundary"] ''
      (
        export PATH=${path}:$PATH
        run ${escapeShellArg src}/install.sh --vault ${escapeShellArg cfg.vault} --relink --no-bin-link${lib.optionalString (!cfg.antigravity) " --no-antigravity"}
      )
    '';
  };
}
