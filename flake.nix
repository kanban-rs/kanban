{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    crane,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        lib = pkgs.lib;

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-analyzer" "clippy" "rustfmt" "llvm-tools-preview"];
        };

        # crane drives the fast, incremental per-PR test check. Dependencies are
        # compiled once into cargoArtifacts (keyed on Cargo.lock) and reused, so
        # CI only recompiles the first-party crates on each change. The released
        # package stays on rustPlatform (default.nix) for nixpkgs parity.
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # kanban-persistence-sqlite pulls schema.sql in via include_str!, so keep
        # .sql files that crane's default source cleaner would otherwise strip.
        sqlFilter = path: _type: builtins.match ".*\\.sql$" path != null;
        srcFilter = path: type:
          (sqlFilter path type) || (craneLib.filterCargoSources path type);
        craneSrc = lib.cleanSourceWith {
          src = self;
          filter = srcFilter;
          name = "source";
        };

        commonArgs = {
          src = craneSrc;
          strictDeps = true;
          pname = "kanban-workspace";
          version = (lib.importTOML ./Cargo.toml).workspace.package.version;
          cargoExtraArgs = "--workspace --all-features";
          nativeBuildInputs = [pkgs.pkg-config];
          buildInputs =
            lib.optionals pkgs.stdenv.isLinux [pkgs.wayland pkgs.xorg.libxcb]
            ++ lib.optionals pkgs.stdenv.isDarwin [pkgs.apple-sdk];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;

        workspaceTests = craneLib.cargoTest (commonArgs
          // {
            inherit cargoArtifacts;
          });

        changeset = pkgs.writeShellApplication {
          name = "changeset";
          runtimeInputs = with pkgs; [coreutils];
          text = builtins.readFile ./scripts/create-changeset.sh;
        };

        bumpVersion = pkgs.writeShellApplication {
          name = "bump-version";
          runtimeInputs = with pkgs; [rustToolchain cargo coreutils gnugrep gnused git findutils];
          text = builtins.readFile ./scripts/bump-version.sh;
        };

        publishCrates = pkgs.writeShellApplication {
          name = "publish-crates";
          runtimeInputs = [rustToolchain pkgs.cargo pkgs.coreutils pkgs.curl pkgs.gnugrep validateRelease listCrates];
          text = builtins.readFile ./scripts/publish-crates.sh;
        };

        aggregateChangelog = pkgs.writeShellApplication {
          name = "aggregate-changelog";
          runtimeInputs = with pkgs; [coreutils gnugrep gnused git findutils];
          text = builtins.readFile ./scripts/aggregate-changelog.sh;
        };

        listCrates = pkgs.writeShellApplication {
          name = "list-crates";
          runtimeInputs = with pkgs; [rustToolchain cargo coreutils jq gnused];
          text = builtins.readFile ./scripts/list-crates.sh;
        };

        validateRelease = pkgs.writeShellApplication {
          name = "validate-release";
          runtimeInputs = with pkgs; [rustToolchain cargo coreutils gnugrep gnused listCrates];
          text = builtins.readFile ./scripts/validate-release.sh;
        };

        checkCrateListSync = pkgs.writeShellApplication {
          name = "check-crate-list-sync";
          runtimeInputs = with pkgs; [coreutils gnugrep findutils diffutils listCrates];
          text = builtins.readFile ./scripts/check-crate-list-sync.sh;
        };

        checkFactoryCompileLock = pkgs.writeShellApplication {
          name = "check-factory-compile-lock";
          runtimeInputs = with pkgs; [coreutils gnugrep gawk ripgrep findutils];
          text = builtins.readFile ./scripts/check-factory-compile-lock.sh;
        };

        kanban = pkgs.callPackage ./default.nix { src = self; gitRev = self.rev or null; };
      in {
        devShells.default = import ./shell.nix {
          inherit pkgs rustToolchain;
          inherit changeset aggregateChangelog bumpVersion publishCrates validateRelease listCrates checkCrateListSync checkFactoryCompileLock;
        };

        devShells.demo = import ./demo/shell.nix { inherit pkgs kanban; };

        checks.tests = workspaceTests;

        packages = let
          kanban-cli = pkgs.callPackage ./default.nix { src = self; gitRev = self.rev or null; withTui = false; };
        in {
          default = kanban;
          kanban-cli = kanban-cli;
          kanban-mcp = pkgs.callPackage ./crates/kanban-mcp/default.nix { src = self; gitRev = self.rev or null; };
          kanban-server = pkgs.callPackage ./crates/kanban-server/default.nix { src = self; gitRev = self.rev or null; };
          kanban-web = pkgs.callPackage ./web/default.nix {};
          aggregate-changelog = aggregateChangelog;
          bump-version = bumpVersion;
          publish-crates = publishCrates;
          validate-release = validateRelease;
          list-crates = listCrates;
          check-crate-list-sync = checkCrateListSync;
          check-factory-compile-lock = checkFactoryCompileLock;
          changeset = changeset;
        };
      }
    );
}
