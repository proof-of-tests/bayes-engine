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
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (system:
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

        # Common source filtering - include static, public, JS files, and e2e_tests for assets and worker code
        src = pkgs.lib.cleanSourceWith {
          src = ./.;
          filter = path: type:
            let
              basePath = baseNameOf path;
            in
            (craneLib.filterCargoSources path type) ||
            (builtins.match ".*/static/.*" path != null) ||
            (builtins.match ".*/public/.*" path != null) ||
            (builtins.match ".*\\.js$" path != null) ||
            # Include e2e_tests directory and its contents
            (basePath == "e2e_tests" && type == "directory") ||
            (builtins.match ".*/e2e_tests/.*" path != null);
        };

        # Common arguments for all Crane builds
        commonArgs = {
          inherit src;
          strictDeps = true;
          # Exclude e2e_tests from the main build
          cargoExtraArgs = "--workspace --exclude e2e_tests";
        };

        # Build the workspace
        cargoBuild = craneLib.buildPackage (commonArgs // {
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # Don't run tests during the main build
          doCheck = false;
        });

        # Common arguments for wasm builds
        commonArgsWasm = {
          inherit src;
          strictDeps = true;
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          # Exclude e2e_tests from WASM builds (it's native-only)
          cargoExtraArgs = "--workspace --exclude e2e_tests";
        };

        # Step 1: Build WASM files (client and server) with cached dependencies
        wasmBuild = craneLibWasm.buildPackage (commonArgsWasm // {
          cargoArtifacts = craneLibWasm.buildDepsOnly commonArgsWasm;

          # We're not installing cargo binaries, we're copying WASM artifacts
          doNotPostBuildInstallCargoBinaries = true;

          # Can't run wasm tests natively
          doCheck = false;

          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";

          installPhaseCommand = ''
            mkdir -p $out
            cp target/wasm32-unknown-unknown/release/client.wasm $out/
            cp target/wasm32-unknown-unknown/release/server.wasm $out/
          '';
        });

        # Step 2: Post-process WASM files with wasm-bindgen and wasm-opt
        webapp = pkgs.stdenv.mkDerivation {
          name = "bayes-engine-webapp";
          inherit src;

          nativeBuildInputs = [
            pkgs.wasm-bindgen-cli
            pkgs.binaryen
            pkgs.findutils
            pkgs.coreutils
          ];

          buildPhase = ''
            ${pkgs.bash}/bin/bash ${./nix/process-wasm.sh} ${wasmBuild}
          '';

          installPhase = ''
            mkdir -p $out
            cp -r build/* $out/
            cp -r assets $out/
          '';
        };
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

          # Build webapp with wasm
          webapp-build = webapp;
        };

        # Packages
        packages = {
          default = cargoBuild;
          bayes-engine = cargoBuild;
          inherit webapp wasmBuild;
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

          # Report sizes of WASM and static assets
          report-sizes = {
            type = "app";
            program = "${pkgs.writeShellScript "report-sizes" ''
              export PATH="${pkgs.lib.makeBinPath [ pkgs.coreutils pkgs.findutils ]}:$PATH"
              exec ${pkgs.bash}/bin/bash ${./nix/report-sizes.sh} ${webapp}
            ''}";
          };

          # Run end-to-end tests
          run-e2e-tests = {
            type = "app";
            program = "${pkgs.writeShellScript "run-e2e-tests" ''
              set -e

              # Build e2e_tests binary
              echo "Building e2e_tests..."
              ${rustToolchain}/bin/cargo build --package e2e_tests

              # Ensure webapp is built (creates result/ directory)
              echo "Ensuring webapp is built..."
              ${pkgs.nix}/bin/nix build .#webapp --no-link || true

              # Start wrangler dev with e2e environment in background
              echo "Starting wrangler dev with e2e environment..."
              WRANGLER_PORT=''${WRANGLER_PORT:-8787}
              ${wrangler.packages.${system}.default}/bin/wrangler dev --env e2e --port $WRANGLER_PORT &
              WRANGLER_PID=$!

              # Wait for wrangler to start (up to 30 seconds)
              echo "Waiting for wrangler to start on port $WRANGLER_PORT..."
              for i in {1..30}; do
                if ${pkgs.curl}/bin/curl -sf http://localhost:$WRANGLER_PORT > /dev/null 2>&1; then
                  echo "Wrangler is ready!"
                  break
                fi
                if [ $i -eq 30 ]; then
                  echo "Wrangler failed to start within 30 seconds"
                  kill $WRANGLER_PID 2>/dev/null || true
                  exit 1
                fi
                sleep 1
              done

              # Run e2e tests
              echo "Running e2e tests..."
              export WRANGLER_PORT
              export WEBDRIVER_PORT=''${WEBDRIVER_PORT:-4444}
              export E2E_BROWSER=''${E2E_BROWSER:-safari}

              # Run tests and capture exit code
              target/debug/e2e_tests
              TEST_EXIT_CODE=$?

              # Cleanup: kill wrangler
              echo "Cleaning up..."
              kill $WRANGLER_PID 2>/dev/null || true

              # Exit with test exit code
              exit $TEST_EXIT_CODE
            ''}";
          };
        };

        # Add a formatter for convenience
        formatter = pkgs.nixpkgs-fmt;
      });
}
