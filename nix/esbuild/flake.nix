{
  description = "esbuild 0.25.10 for CloudFlare Workers";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachSystem [ "x86_64-linux" "aarch64-linux" "aarch64-darwin" ] (system:
      let
        pkgs = import nixpkgs { inherit system; };

        esbuild_0_25_10 = pkgs.stdenv.mkDerivation rec {
          pname = "esbuild";
          version = "0.25.10";

          src =
            if pkgs.stdenv.isLinux then
              (if pkgs.stdenv.isAarch64 then
                pkgs.fetchurl
                  {
                    url = "https://registry.npmjs.org/@esbuild/linux-arm64/-/linux-arm64-${version}.tgz";
                    hash = "sha256-6q8Hh3OZvJ9nhJLf/R62VdeQKgm6XjbTq28xYysS9dU=";
                  }
              else
                pkgs.fetchurl {
                  url = "https://registry.npmjs.org/@esbuild/linux-x64/-/linux-x64-${version}.tgz";
                  hash = "sha256-Jae5aLjlFyuqqPRPkbccHS1+dgBCxpHyKrWVJ9hw0UU=";
                })
            else if pkgs.stdenv.isDarwin then
              (if pkgs.stdenv.isAarch64 then
                pkgs.fetchurl
                  {
                    url = "https://registry.npmjs.org/@esbuild/darwin-arm64/-/darwin-arm64-${version}.tgz";
                    hash = "sha256-3TOeNykqcRsuulRvsd823Z+EaoSaw+39OsUnG1Ai8yM=";
                  }
              else throw "Unsupported Darwin platform (only aarch64-darwin is supported)")
            else throw "Unsupported platform";

          dontUnpack = false;
          unpackPhase = ''
            tar xzf $src
          '';

          installPhase = ''
            mkdir -p $out/bin
            cp package/bin/esbuild $out/bin/
            chmod +x $out/bin/esbuild
          '';
        };
      in
      {
        packages = {
          default = esbuild_0_25_10;
          esbuild = esbuild_0_25_10;
        };
      });
}
