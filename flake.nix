{
  description = "zmqtt2prom-rs - MQTT to Prometheus bridge";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    crane.url = "github:ipetkov/crane";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs =
    {
      self,
      nixpkgs,
      crane,
      flake-utils,
      ...
    }:
    flake-utils.lib.eachDefaultSystem (
      system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        craneLib = crane.mkLib pkgs;

        # Common arguments for all builds
        commonArgs = {
          src = craneLib.cleanCargoSource ./.;
          strictDeps = true;
        };

        # Build only dependencies for caching
        cargoArtifacts = craneLib.buildDepsOnly (
          commonArgs
          // {
            pname = "zmqtt2prom-rs-deps";
          }
        );

        # Build the actual package
        zmqtt2prom-rs = craneLib.buildPackage (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );

        # Clippy check
        zmqtt2prom-rs-clippy = craneLib.cargoClippy (
          commonArgs
          // {
            inherit cargoArtifacts;
            cargoClippyExtraArgs = "--all-targets -- --deny warnings";
          }
        );

        # Run tests
        zmqtt2prom-rs-test = craneLib.cargoTest (
          commonArgs
          // {
            inherit cargoArtifacts;
          }
        );
      in
      {
        packages = {
          default = zmqtt2prom-rs;
          zmqtt2prom-rs = zmqtt2prom-rs;
        };

        checks = {
          inherit zmqtt2prom-rs zmqtt2prom-rs-clippy zmqtt2prom-rs-test;
        };

        devShells.default = craneLib.devShell {
          checks = self.checks.${system};

          inputsFrom = [ zmqtt2prom-rs ];

          packages = with pkgs; [
            rust-analyzer
            cargo-watch
            cargo-edit
          ];
        };
      }
    );
}
