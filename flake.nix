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
              buildInputs = [ pkgs.mdformat ];
            } ''
            cd ${./.}
            # Find all markdown files and check if they're formatted
            find . -name "*.md" -type f | while read -r file; do
              echo "Checking $file..."
              ${pkgs.mdformat}/bin/mdformat --check "$file" || {
                echo "Error: $file is not properly formatted"
                echo "Run 'nix run nixpkgs#mdformat -- --wrap 80 $file' to fix"
                exit 1
              }
            done
            touch $out
          '';

          # Check Nix formatting with nixpkgs-fmt
          nix-format = pkgs.runCommand "check-nix-format"
            {
              buildInputs = [ pkgs.nixpkgs-fmt ];
            } ''
            cd ${./.}
            # Find all nix files and check if they're formatted
            find . -name "*.nix" -type f | while read -r file; do
              echo "Checking $file..."
              ${pkgs.nixpkgs-fmt}/bin/nixpkgs-fmt --check "$file" || {
                echo "Error: $file is not properly formatted"
                echo "Run 'nix fmt' to fix formatting"
                exit 1
              }
            done
            touch $out
          '';

          # Check Nix lints with statix
          nix-lint = pkgs.runCommand "check-nix-lint"
            {
              buildInputs = [ pkgs.statix ];
            } ''
            cd ${./.}
            # Find all nix files and run statix
            echo "Running statix checks..."
            ${pkgs.statix}/bin/statix check ${./.} || {
              echo "Error: Nix files have lint issues"
              echo "Run 'nix run nixpkgs#statix check' to see issues"
              echo "Run 'nix run nixpkgs#statix fix' to auto-fix"
              exit 1
            }
            touch $out
          '';
        };

        # Add a formatter for convenience
        formatter = pkgs.nixpkgs-fmt;
      });
}
