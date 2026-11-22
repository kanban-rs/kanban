{
  lib,
  pkgs,
  rustPlatform,
  src,
  gitRev ? null,
  withTui ? true,
}:

let
  cargoToml = lib.importTOML ./Cargo.toml;
in
rustPlatform.buildRustPackage {
  pname = "kanban";
  inherit (cargoToml.workspace.package) version;

  inherit src;

  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs =
    lib.optionals (pkgs.stdenv.isLinux && withTui) [
      pkgs.wayland
      pkgs.xorg.libxcb
    ]
    ++ lib.optionals (pkgs.stdenv.isDarwin && withTui) [
      pkgs.apple-sdk
    ];

  cargoBuildFlags = [ "--package" "kanban-cli" ]
    ++ lib.optionals (!withTui) [ "--no-default-features" ];
  doCheck = false;

  preBuild = lib.optionalString (gitRev != null) ''
    export GIT_COMMIT_HASH="${gitRev}"
  '';

  meta = {
    inherit (cargoToml.workspace.package) description homepage;
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ fulsomenko ];
    mainProgram = "kanban";
    platforms = lib.platforms.all;
  };
}
