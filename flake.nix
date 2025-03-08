{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs =
    {
      self,
      nixpkgs,
      flake-utils,
      rust-overlay,
      crane,
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        version = "0.3.1";

        overlays = [ rust-overlay.overlays.default ];
        pkgs = import nixpkgs { inherit system overlays; };

        rust = pkgs.rust-bin.fromRustupToolchainFile ./server/rust-toolchain.toml;
        craneLib = (crane.mkLib nixpkgs.legacyPackages.${system}).overrideToolchain rust;

        commonArgs = {
          src = pkgs.lib.cleanSourceWith {
            src = ./server;
            filter = path: type: craneLib.filterCargoSources path type || builtins.match ".*sql$" path != null;
            name = "source";
          };

          nativeBuildInputs = with pkgs; [
            pkg-config
            rustPlatform.bindgenHook
          ];

          buildInputs = with pkgs; [
            ffmpeg
            openssl
            postgresql
          ];
        };

        cargoArtifacts = craneLib.buildDepsOnly commonArgs;
      in
      rec {
        devShells.default = craneLib.devShell {
          packages =
            with pkgs;
            [
              git
            ]
            ++ packages.oxibooru.server.buildInputs
            ++ packages.oxibooru.server.nativeBuildInputs;

          RUSTDOCFLAGS = "--cfg docsrs -D warnings";
        };

        packages = rec {
          oxibooru = pkgs.stdenv.mkDerivation {
            client = pkgs.buildNpmPackage {
              pname = "oxibooru-client";
              inherit version;

              src = ./client;

              npmDepsHash = "sha256-OcembsqQHrLyAdBvEdcYVwUCm+4zQ2QkoNyxP2LvgVA=";
              makeCacheWritable = true;

              npmBuildFlags = [
                "--gzip"
              ];

              installPhase = ''
                runHook preInstall

                mkdir $out
                mv ./public/* $out

                runHook postInstall
              '';
            };

            server = craneLib.buildPackage (
              commonArgs
              // {
                inherit cargoArtifacts;
              }
            );
          };

          default = oxibooru;
        };

        checks.oxibooru = packages.oxibooru;

        checks.oxibooru-clippy = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      }
    );
}
