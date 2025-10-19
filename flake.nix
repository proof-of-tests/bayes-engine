{
  description = "Bayes Engine - A Bayesian inference project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    crane.url = "github:ipetkov/crane";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    wrangler = {
      url = "github:emrldnix/wrangler";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    advisory-db = {
      url = "github:rustsec/advisory-db";
      flake = false;
    };
  };

  outputs = { self, nixpkgs, flake-utils, crane, rust-overlay, wrangler, advisory-db }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs {
          inherit system;
          overlays = [ rust-overlay.overlays.default ];
        };

        # Load Rust toolchain from rust-toolchain.toml
        rustToolchain = pkgs.rust-bin.fromRustupToolchainFile ./rust-toolchain.toml;

        # Rust toolchain with wasm32 target for webapp builds
        rustToolchainWasm = rustToolchain.override {
          targets = [ "wasm32-unknown-unknown" ];
        };

        # Create craneLib with custom toolchain
        craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

        # Create craneLib for wasm builds
        craneLibWasm = (crane.mkLib pkgs).overrideToolchain rustToolchainWasm;

        # Common source filtering
        src = craneLib.cleanCargoSource ./.;

        # Common arguments for all Crane builds
        commonArgs = {
          inherit src;
          strictDeps = true;
        };

        # Build the workspace
        cargoBuild = craneLib.buildPackage (commonArgs // {
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
        });

        # Common arguments for wasm builds
        commonArgsWasm = {
          inherit src;
          strictDeps = true;
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          nativeBuildInputs = [
            pkgs.worker-build
            pkgs.wasm-bindgen-cli
            pkgs.binaryen
            pkgs.nodejs_22
          ];
        };

        # Build the webapp with worker-build
        webapp = craneLibWasm.buildPackage (commonArgsWasm // {
          cargoArtifacts = craneLibWasm.buildDepsOnly commonArgsWasm;

          # We're not installing cargo binaries, we're copying build artifacts
          doNotPostBuildInstallCargoBinaries = true;

          # Can't run wasm tests natively
          doCheck = false;

          # Make source writable since worker-build creates a build directory
          postUnpack = ''
            chmod -R +w $sourceRoot
          '';

          preBuild = ''
            # worker-build needs writable directories
            export HOME=$TMPDIR
            mkdir -p $HOME/.cache
          '';

          buildPhaseCargoCommand = ''
            worker-build --release
          '';

          installPhaseCommand = ''
            mkdir -p $out
            cp -r build/* $out/
          '';
        });
      in
      {
        checks = {
          # Check markdown formatting
          markdown-format = pkgs.runCommand "check-markdown-format"
            {
              buildInputs = [ pkgs.mdformat pkgs.bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-markdown-format.sh} ${./.}
            touch $out
          '';

          # Check Nix formatting with nixpkgs-fmt
          nix-format = pkgs.runCommand "check-nix-format"
            {
              buildInputs = [ pkgs.nixpkgs-fmt pkgs.bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-nix-format.sh} ${./.}
            touch $out
          '';

          # Check Nix lints with statix
          nix-lint = pkgs.runCommand "check-nix-lint"
            {
              buildInputs = [ pkgs.statix pkgs.bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-nix-lint.sh} ${./.}
            touch $out
          '';

          # Check shell scripts with shellcheck
          shellcheck = pkgs.runCommand "check-shellcheck"
            {
              buildInputs = [ pkgs.shellcheck pkgs.bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-shellcheck.sh} ${./.}
            touch $out
          '';

          # Build Rust website with Crane
          rust-build = cargoBuild;

          # Check Rust formatting with Crane
          rust-fmt = craneLib.cargoFmt {
            inherit (commonArgs) src;
          };

          # Check Rust with Clippy using Crane
          rust-clippy = craneLib.cargoClippy (commonArgs // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          });

          # Run Rust tests with Crane
          rust-test = craneLib.cargoTest (commonArgs // {
            cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          });

          # Run cargo audit with Crane
          rust-audit = craneLib.cargoAudit {
            inherit (commonArgs) src;
            inherit advisory-db;
          };
        };

        # Packages
        packages = {
          default = cargoBuild;
          bayes-engine = cargoBuild;
          inherit webapp;
        };

        apps = {
          # Deploy the app locally using wrangler
          # Run this from a directory containing wrangler.toml
          wrangler-dev = {
            type = "app";
            program = "${pkgs.writeShellScript "wrangler-dev" ''
              exec ${wrangler.packages.${system}.default}/bin/wrangler dev
            ''}";
          };
        };

        # Add a formatter for convenience
        formatter = pkgs.nixpkgs-fmt;
      });
}
