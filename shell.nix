{ pkgs ? import <nixpkgs> {}, rustToolchain ? pkgs.rustc }:

pkgs.mkShell {
  name = "kanban-rust-shell";

  buildInputs = with pkgs; [
    # Rust toolchain
    rustToolchain
    cargo-watch
    cargo-edit
    cargo-audit
    cargo-tarpaulin

    # Build dependencies
    pkg-config
    openssl

    # Development utilities
    bacon
  ];

  shellHook = ''
    export RUST_BACKTRACE=1
    echo "Kanban Development Environment"
    echo "📦 Cargo: $(cargo --version)"
    echo "🦀 Rustc: $(rustc --version)"
  '';
}

