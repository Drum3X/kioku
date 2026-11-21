{
  lib,
  rustPlatform,
  git,
  onnxruntime,
}: let
  # What the binary calls at run time. Hooks run from the agents, so the Home Manager module
  # also puts these on the user's PATH.
  runtimeDeps = [git];
in
  rustPlatform.buildRustPackage {
    pname = "kioku";
    version = lib.removeSuffix "\n" (builtins.readFile ../VERSION);

    src = lib.fileset.toSource {
      root = ../.;
      fileset = lib.fileset.unions [
        ../Cargo.toml
        ../Cargo.lock
        ../src
        ../skills
        ../hermes-plugin
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
    cargoLock.lockFile = ../Cargo.lock;

    # Hybrid search with nixpkgs' ONNX Runtime, loaded at run time from the path baked in here
    # (the sandbox cannot download the linked-in one). Search stays off until kioku semantic on.
    buildFeatures = ["semantic-dynamic"];
    env.KIOKU_ORT_LIB_DEFAULT = "${onnxruntime}/lib/libonnxruntime.so";

    # share/kioku is what a vault's .kioku link points to: the system files with the binary in
    # share/kioku/bin/kioku. bin/kioku is the same binary (it finds the vault through the
    # install record); bin/kioku-install runs `kioku install`.
    postInstall = ''
      mkdir -p $out/share/kioku/bin
      cp -r skills skeleton templates hermes-plugin install.sh SCHEMA.md README.md README.tr.md LICENSE VERSION $out/share/kioku/
      mv $out/bin/kioku $out/share/kioku/bin/kioku
      ln -s $out/share/kioku/bin/kioku $out/bin/kioku
      cat > $out/bin/kioku-install <<SH
      #!/bin/sh
      export PATH=${lib.makeBinPath runtimeDeps}:\$PATH
      exec $out/share/kioku/bin/kioku install "\$@"
      SH
      chmod +x $out/bin/kioku-install
    '';

    passthru = {inherit runtimeDeps;};

    meta = {
      description = "A personal second brain kept by your AI agents: memory, an LLM wiki and a journal in a Markdown vault";
      homepage = "https://github.com/Drum3X/kioku";
      license = lib.licenses.gpl3Only;
      mainProgram = "kioku";
      platforms = lib.platforms.unix;
    };
  }
