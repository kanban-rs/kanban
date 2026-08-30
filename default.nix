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

  cargoHash = "sha256-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";

  nativeBuildInputs = [ pkgs.pkg-config ];
  buildInputs = lib.optionals (pkgs.stdenv.isLinux && withTui) [
    pkgs.wayland
    pkgs.xorg.libxcb
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
