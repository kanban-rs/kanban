{ lib
, rustPlatform
, pkg-config
, openssl
, stdenv
, darwin
}:

rustPlatform.buildRustPackage {
  pname = "kanban";
  version = "0.1.0";

  src = ./.;

  cargoLock = {
    lockFile = ./Cargo.lock;
  };

  nativeBuildInputs = [
    pkg-config
  ];

  buildInputs = [
    openssl
  ] ++ lib.optionals stdenv.isDarwin [
    darwin.apple_sdk.frameworks.Security
  ];

  meta = with lib; {
    description = "A terminal-based kanban board";
    homepage = "https://github.com/fulsomenko/kanban";
    license = licenses.asl20;
    maintainers = [ ];
    mainProgram = "kanban";
    platforms = platforms.all;
  };
}
