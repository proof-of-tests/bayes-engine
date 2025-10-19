{
  description = "Bayes Engine - A Bayesian inference project";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
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

          # Build Rust website
          rust-build = pkgs.runCommand "check-rust-build"
            {
              buildInputs = with pkgs; [ rustc cargo bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-rust-build.sh} ${./.}
            touch $out
          '';

          # Check Rust formatting
          rust-fmt = pkgs.runCommand "check-rust-fmt"
            {
              buildInputs = with pkgs; [ rustc cargo rustfmt bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-rust-fmt.sh} ${./.}
            touch $out
          '';

          # Check Rust with Clippy
          rust-clippy = pkgs.runCommand "check-rust-clippy"
            {
              buildInputs = with pkgs; [ rustc cargo clippy bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-rust-clippy.sh} ${./.}
            touch $out
          '';

          # Run Rust tests
          rust-test = pkgs.runCommand "check-rust-test"
            {
              buildInputs = with pkgs; [ rustc cargo bash ];
            } ''
            ${pkgs.bash}/bin/bash ${./nix/check-rust-test.sh} ${./.}
            touch $out
          '';
        };

        # Add a formatter for convenience
        formatter = pkgs.nixpkgs-fmt;
      });
}
