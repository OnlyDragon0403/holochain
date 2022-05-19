{ nixpkgs ? null
, rustVersion ? {
    track = "stable";
    version = "1.60.0";
  }

, holonixArgs ? {
    inherit rustVersion;
  }
}:

# This is an example of what downstream consumers of holonix should do
# This is also used to dogfood as many commands as possible for holonix
# For example the release process for holonix uses this file
let
  # point this to your local config.nix file for this project
  # example.config.nix shows and documents a lot of the options
  config = import ./config.nix;
  sources = import ./nix/sources.nix;

  # START HOLONIX IMPORT BOILERPLATE
  holonixPath = config.holonix.pathFn { };
  holonix = config.holonix.importFn holonixArgs;
  # END HOLONIX IMPORT BOILERPLATE

  overlays = [
    (self: super: {
      inherit holonix holonixPath;

      hcToplevelDir = builtins.toString ./.;

      nixEnvPrefixEval = ''
        if [[ -n "$NIX_ENV_PREFIX" ]]; then
          # don't touch it
          :
        elif test -w "$PWD"; then
          export NIX_ENV_PREFIX="$PWD"
        elif test -d "${builtins.toString self.hcToplevelDir}" &&
            test -w "${builtins.toString self.hcToplevelDir}"; then
          export NIX_ENV_PREFIX="${builtins.toString self.hcToplevelDir}"
        elif test -d "$HOME" && test -w "$HOME"; then
          export NIX_ENV_PREFIX="$HOME/.cache/holochain-dev"
          mkdir -p "$NIX_ENV_PREFIX"
        else
          export NIX_ENV_PREFIX="$(${self.coreutils}/bin/mktemp -d)"
        fi
      '';

      crate2nix = import sources.crate2nix.outPath { };

      cargo-nextest = self.rustPlatform.buildRustPackage {
        name = "cargo-nextest";

        src = sources.nextest.outPath;
        cargoSha256 = "sha256-1siQ7Ar1priIeHTEG3+2RmNLK1u92CdKvciD1oMH2sw=";

        cargoTestFlags = [
          # TODO: investigate some more why these tests fail in nix
          "--"
          "--skip=tests_integration::test_relocated_run"
          "--skip=tests_integration::test_run"
          "--skip=tests_integration::test_run_after_build"
        ];
      };
    })

  ];

  nixpkgs' = import (nixpkgs.path or holonix.pkgs.path) { inherit overlays; };
  inherit (nixpkgs') callPackage;

  pkgs = callPackage ./nix/pkgs/default.nix { };
in
{
  inherit
    nixpkgs'
    holonix
    pkgs
    ;

  # TODO: refactor when we start releasing again
  # releaseHooks = callPackages ./nix/release {
  #   inherit
  #     config
  #     nixpkgs
  #     ;
  # };

  shells = callPackage ./nix/shells.nix {
    inherit
      holonix
      pkgs
      ;
  };
}
