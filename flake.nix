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
          config.allowUnfree = true;
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
          # Exclude e2e_tests and wasm-only helper crates from the main build
          cargoExtraArgs = "--workspace --exclude e2e_tests --exclude simple-wasm-module --exclude pow-test-functions";
        };

        # Build the workspace
        cargoBuild = craneLib.buildPackage (commonArgs // {
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          # Don't run tests during the main build
          doCheck = false;
        });

        # Build e2e_tests binary separately (native-only, for testing)
        e2eTests = craneLib.buildPackage (commonArgs // {
          cargoArtifacts = craneLib.buildDepsOnly commonArgs;
          pname = "e2e_tests";
          cargoExtraArgs = "--package e2e_tests";
          doCheck = false;
        });

        # Build simple-wasm-module separately for e2e testing
        simpleWasmModule = craneLibWasm.buildPackage {
          inherit src;
          strictDeps = true;
          pname = "simple-wasm-module";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          cargoExtraArgs = "--package simple-wasm-module";

          # Build only dependencies first
          cargoArtifacts = craneLibWasm.buildDepsOnly {
            inherit src;
            strictDeps = true;
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            cargoExtraArgs = "--package simple-wasm-module";
          };

          doCheck = false;
          doNotPostBuildInstallCargoBinaries = true;

          installPhaseCommand = ''
            mkdir -p $out
            cp target/wasm32-unknown-unknown/release/simple_wasm_module.wasm $out/
          '';
        };

        # Build pow-test-functions separately as uploadable test WASM module
        powTestFunctionsModule = craneLibWasm.buildPackage {
          inherit src;
          strictDeps = true;
          pname = "pow-test-functions";
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          cargoExtraArgs = "--package pow-test-functions";

          cargoArtifacts = craneLibWasm.buildDepsOnly {
            inherit src;
            strictDeps = true;
            CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
            cargoExtraArgs = "--package pow-test-functions";
          };

          doCheck = false;
          doNotPostBuildInstallCargoBinaries = true;

          installPhaseCommand = ''
            mkdir -p $out
            cp target/wasm32-unknown-unknown/release/pow_test_functions.wasm $out/
          '';
        };

        # Common arguments for wasm builds
        commonArgsWasm = {
          inherit src;
          strictDeps = true;
          CARGO_BUILD_TARGET = "wasm32-unknown-unknown";
          # Exclude e2e_tests and standalone wasm helper crates (built separately)
          cargoExtraArgs = "--workspace --exclude e2e_tests --exclude simple-wasm-module --exclude pow-test-functions";
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
          inherit webapp wasmBuild e2eTests simpleWasmModule powTestFunctionsModule;
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
              export PATH="${pkgs.lib.makeBinPath [ pkgs.util-linux pkgs.procps pkgs.coreutils pkgs.geckodriver ]}:$PATH"
              export WEBAPP_PATH="${webapp}"
              export CURL_BIN="${pkgs.curl}/bin/curl"
              export WRANGLER_BIN="${wrangler.packages.${system}.default}/bin/wrangler"
              export E2E_TESTS_BIN="${e2eTests}/bin/e2e_tests"
              export SIMPLE_WASM_MODULE="${simpleWasmModule}/simple_wasm_module.wasm"
              exec ${pkgs.bash}/bin/bash ${./nix/run-e2e-tests.sh}
            ''}";
          };
        };

        # Add a formatter for convenience
        formatter = pkgs.nixpkgs-fmt;
      });
}
