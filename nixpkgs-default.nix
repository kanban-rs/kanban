# This is the default.nix file adapted for nixpkgs submission
# Place this in pkgs/applications/terminal-ui/kanban/default.nix in the nixpkgs repo
#
# Usage:
# 1. Copy to: nixpkgs/pkgs/applications/terminal-ui/kanban/default.nix
# 2. Calculate hashes by running:
#    nix-build -A kanban 2>&1 | grep "got:"
# 3. Replace hash values with actual values from step 2

{ lib
, rustPlatform
, fetchFromGitHub
, nix-update-script
}:

rustPlatform.buildRustPackage rec {
  pname = "kanban";
  version = "0.1.10";

  src = fetchFromGitHub {
    owner = "fulsomenko";
    repo = "kanban";
    rev = "v${version}";
    # To get the correct hash:
    # nix hash file (downloaded tarball) --type sha256 --base64
    # Or build once with: hash = lib.fakeHash; then use the error message
    hash = "sha256-PLACEHOLDER_SRC_HASH";
  };

  # cargoHash calculated from Cargo.lock
  # Get by running: nix-build -A kanban 2>&1 | grep "got:"
  cargoHash = "sha256-PLACEHOLDER_CARGO_HASH";

  meta = {
    description = "A terminal-based project management solution";
    longDescription = ''
      A terminal-based kanban/project management tool inspired by lazygit,
      built with Rust. Features async/await runtime, fast terminal UI with
      ratatui, file persistence with JSON import/export, keyboard-driven
      navigation with vim-like shortcuts, and comprehensive task management
      including sprints, story points, and metadata tracking.
    '';
    homepage = "https://github.com/fulsomenko/kanban";
    downloadPage = "https://github.com/fulsomenko/kanban/releases";
    license = lib.licenses.asl20;
    maintainers = with lib.maintainers; [ fulsomenko ];
    mainProgram = "kanban";
    platforms = lib.platforms.all;
  };

  passthru.updateScript = nix-update-script { };
}
