{
  lib,
  stdenvNoCC,
  makeWrapper,
  bash,
  coreutils,
  findutils,
  gawk,
  git,
  gnugrep,
  gnused,
  jq,
  libsecret,
  nodejs,
  ripgrep,
}: let
  # What the scripts call. Hooks run from Claude Code, so the Home Manager module also
  # puts these on the user's PATH.
  runtimeDeps =
    [bash coreutils findutils gawk git gnugrep gnused jq nodejs ripgrep]
    # secret-tool, for kioku password (Secret Service); macOS has its own Keychain tool.
    ++ lib.optional stdenvNoCC.hostPlatform.isLinux libsecret;
in
  stdenvNoCC.mkDerivation {
    pname = "kioku";
    version = lib.removeSuffix "\n" (builtins.readFile ../VERSION);

    src = lib.fileset.toSource {
      root = ../.;
      fileset = lib.fileset.unions [
        ../bin
        ../hooks
        ../skills
        ../skeleton
        ../templates
        ../install.sh
        ../SCHEMA.md
        ../README.md
        ../README.tr.md
        ../LICENSE
        ../VERSION
      ];
    };

    nativeBuildInputs = [makeWrapper];
    buildInputs = [bash];
    dontConfigure = true;
    dontBuild = true;

    # share/kioku is what a vault's .kioku link points to. bin/kioku runs the CLI of the
    # vault in $KIOKU_DIR (default ~/Documents/Kioku); bin/kioku-install is install.sh.
    installPhase = ''
      runHook preInstall
      mkdir -p $out/share/kioku $out/bin
      cp -r . $out/share/kioku/
      chmod +x $out/share/kioku/bin/kioku $out/share/kioku/bin/kioku-version $out/share/kioku/hooks/*.sh $out/share/kioku/install.sh
      patchShebangs $out/share/kioku/bin $out/share/kioku/hooks $out/share/kioku/install.sh
      makeWrapper $out/share/kioku/install.sh $out/bin/kioku-install \
        --prefix PATH : ${lib.makeBinPath runtimeDeps}
      cat > $out/bin/kioku <<SH
      #!${bash}/bin/bash
      export PATH=${lib.makeBinPath runtimeDeps}:\$PATH
      exec "\''${KIOKU_DIR:-\$HOME/Documents/Kioku}/.kioku/bin/kioku" "\$@"
      SH
      chmod +x $out/bin/kioku
      runHook postInstall
    '';

    passthru = {inherit runtimeDeps;};

    meta = {
      description = "A personal second brain for Claude Code: memory, an LLM wiki and a journal in a Markdown vault";
      homepage = "https://github.com/Drum3X/kioku";
      license = lib.licenses.gpl3Only;
      mainProgram = "kioku";
      platforms = lib.platforms.unix;
    };
  }
