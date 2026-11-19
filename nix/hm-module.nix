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
  path = lib.makeBinPath (cfg.package.runtimeDeps
    ++ lib.optionals cfg.capture.gui [pkgs.kdePackages.kdialog pkgs.libnotify]);

  cli = pkgs.writeShellScriptBin "kioku" ''
    export PATH=${path}:$PATH
    exec ${escapeShellArg cfg.vault}/.kioku/bin/kioku "$@"
  '';
  bin = "${cfg.package}/share/kioku/bin/kioku";
  # `kioku install` by hand, e.g. kioku-install --uninstall before turning the module off.
  # KIOKU_SYS: the vault links to the package, or to the checkout given in `source`.
  installer = pkgs.writeShellScriptBin "kioku-install" ''
    export PATH=${path}:$PATH KIOKU_SYS=${escapeShellArg src}
    exec ${bin} install "$@"
  '';
in {
  options.programs.kioku = {
    enable = mkEnableOption "Kioku, a personal second brain kept by your AI agents";

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

    claudeCode = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Set Kioku up in Claude Code (rules in ~/.claude/CLAUDE.md, skills, hooks in
        ~/.claude/settings.json) when ~/.claude exists. Set false to leave Claude Code alone.
      '';
    };

    codex = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Set Kioku up in OpenAI Codex (rules in ~/.codex/AGENTS.md, skills in ~/.agents/skills,
        hooks in ~/.codex/hooks.json, the vault as a writable sandbox root) when ~/.codex exists.
        Set false to leave Codex alone.
      '';
    };

    antigravity = mkOption {
      type = types.bool;
      default = true;
      description = ''
        Also set Kioku up in Google Antigravity (rules in ~/.gemini/GEMINI.md, skills in
        ~/.gemini/config/skills) when ~/.gemini exists. Set false to leave Antigravity alone.
      '';
    };

    capture.gui = mkOption {
      type = types.bool;
      default = false;
      description = ''
        Make `kioku capture --gui` work: puts kdialog and notify-send on the kioku command's PATH
        and adds a "Kioku capture" desktop entry, which you can bind to a keyboard shortcut.
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

    xdg.desktopEntries = mkIf cfg.capture.gui {
      kioku-capture = {
        name = "Kioku capture";
        comment = "Drop a thought, a todo or a link into the Kioku inbox";
        exec = "${cli}/bin/kioku capture --gui";
        icon = "document-new";
        terminal = false;
        categories = ["Utility"];
      };
    };

    # The agents' wiring lives in files they also edit themselves (settings.json, CLAUDE.md,
    # hooks.json, config.toml …), so it is merged by install.sh rather than owned by Home Manager.
    home.activation.kioku = lib.hm.dag.entryAfter ["writeBoundary"] ''
      (
        export PATH=${path}:$PATH KIOKU_SYS=${escapeShellArg src}
        ${lib.optionalString (cfg.source != null) ''
          # A checkout's bin/kioku is a build output: point it at the package binary unless it
          # is the developer's own build (cargo build --release links it to target/).
          b=${escapeShellArg cfg.source}/bin/kioku
          if [ ! -e "$b" ] || { [ -L "$b" ] && [[ "$(readlink "$b")" == /nix/store/* ]]; }; then
            run ln -sfn ${bin} "$b"
          fi
        ''}
        run ${bin} install --vault ${escapeShellArg cfg.vault} --relink --no-bin-link${lib.optionalString (!cfg.claudeCode) " --no-claude"}${lib.optionalString (!cfg.codex) " --no-codex"}${lib.optionalString (!cfg.antigravity) " --no-antigravity"}
      )
    '';
  };
}
