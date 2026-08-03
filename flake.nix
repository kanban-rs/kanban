{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    rust-overlay.url = "github:oxalica/rust-overlay";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    rust-overlay,
    flake-utils,
    ...
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };

        rustToolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-src" "rust-analyzer" "clippy" "rustfmt"];
        };

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

        checkKanbanViewNoRatatui = pkgs.writeShellApplication {
          name = "check-kanban-view-no-ratatui";
          runtimeInputs = with pkgs; [rustToolchain cargo gnugrep];
          text = builtins.readFile ./scripts/check-kanban-view-no-ratatui.sh;
        };

        kanban = pkgs.callPackage ./default.nix { src = self; gitRev = self.rev or null; };
      in {
        devShells.default = import ./shell.nix {
          inherit pkgs rustToolchain;
          inherit changeset aggregateChangelog bumpVersion publishCrates validateRelease listCrates checkCrateListSync checkFactoryCompileLock checkKanbanViewNoRatatui;
        };

        devShells.demo = import ./demo/shell.nix { inherit pkgs kanban; };

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
          check-kanban-view-no-ratatui = checkKanbanViewNoRatatui;
          changeset = changeset;
        };
      }
    );
}
