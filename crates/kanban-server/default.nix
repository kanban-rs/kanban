{ lib
, rustPlatform
, src
, gitRev ? null
}:

let
  cargoToml = lib.importTOML ../../Cargo.toml;
in
rustPlatform.buildRustPackage {
  inherit (cargoToml.workspace.package) version;
  pname = "kanban-server";

  inherit src;

  cargoLock = {
    lockFile = ../../Cargo.lock;
  };

  preBuild = lib.optionalString (gitRev != null) ''
    export GIT_COMMIT_HASH="${gitRev}"
  '';

  # Only build the kanban-server binary
  cargoBuildFlags = [ "--package" "kanban-server" ];
  cargoTestFlags = [ "--package" "kanban-server" ];

  meta = {
    inherit (cargoToml.workspace.package) description homepage;
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ fulsomenko ];
    mainProgram = "kanban-server";
    platforms = lib.platforms.all;
  };
}
